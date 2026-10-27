use std::collections::{BTreeMap, HashSet};

use axum::extract::State;
use futures::{StreamExt, pin_mut};
use ruma::{
	EventId, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedUserId, RoomId, UserId,
	api::{client::tuwunel::batch_send::unstable as batch_send, error::ErrorKind},
	events::{
		RoomAccountDataEventType, TimelineEventType,
		fully_read::{FullyReadEvent, FullyReadEventContent},
		reaction::ReactionEventContent,
		receipt::{Receipt, ReceiptEvent, ReceiptEventContent, ReceiptThread, ReceiptType},
		room::{
			encrypted::Relation, member::MembershipState, redaction::RoomRedactionEventContent,
		},
	},
};
use serde::Deserialize;
use tuwunel_core::{
	Err, Error, Result, err,
	matrix::{
		event::Event,
		pdu::{PduBuilder, PduEvent},
	},
	warn,
};
use tuwunel_service::{Services, appservice::RegistrationInfo};

use crate::Ruma;

#[derive(Deserialize)]
struct ExtractRelatesTo {
	#[serde(rename = "m.relates_to")]
	relates_to: Relation,
}

/// # `POST /_matrix/client/unstable/com.beeper.backfill/rooms/{roomId}/batch_send`
///
/// Beeper's batch sending extension, used by mautrix bridgev2 to backfill
/// history. Callable only by appservices (enforced by the endpoint's
/// `AppserviceToken` authentication scheme).
///
/// - Every event is sent by its own `sender`, which must be a local user in the
///   appservice's namespace (or a trusted double puppeting appservice's) who is
///   joined to the room.
/// - Events may carry a client-chosen `event_id`, which is stored as-is. This
///   is only allowed in rooms without remote servers, since the ID doesn't
///   match the event's reference hash.
/// - Forward batches are appended to the timeline, backward batches are
///   inserted before the start of it.
pub(crate) async fn batch_send_route(
	State(services): State<crate::State>,
	body: Ruma<batch_send::Request>,
) -> Result<batch_send::Response> {
	let Some(appservice_info) = body.appservice_info.as_ref() else {
		return Err!(Request(MissingToken("Batch sending is only available to appservices.")));
	};

	let room_id = &body.room_id;
	if !services.metadata.exists(room_id).await {
		return Err!(Request(NotFound("Room not found.")));
	}

	if services.admin.is_admin_room(room_id).await {
		return Err!(Request(Forbidden("Batch sending into the admin room is not allowed.")));
	}

	let has_custom_ids = body
		.events
		.iter()
		.any(|event| event.event_id.is_some());

	if has_custom_ids && has_remote_servers(&services, room_id).await {
		return Err!(Request(Forbidden(
			"Custom event IDs are only allowed in rooms without remote servers."
		)));
	}

	for event in &body.events {
		if event.state_key.is_some() {
			return Err!(Request(InvalidParam("State events can't be batch sent.")));
		}

		let sender = &event.sender;
		if !services.globals.user_is_local(sender)
			|| !services
				.appservice
				.can_act_as_user(appservice_info, sender)
				.await
		{
			return Err!(Request(Exclusive(
				"Sender {sender} is not in the appservice's namespace."
			)));
		}

		if services
			.state_cache
			.user_membership(sender, room_id)
			.await
			.is_some_and(|membership| membership == MembershipState::Ban)
		{
			return Err!(Request(Forbidden("Sender {sender} is banned from the room.")));
		}
	}

	let state_lock = services.state.mutex.lock(room_id).await;

	let batch_ids: HashSet<&EventId> = body
		.events
		.iter()
		.filter_map(|event| event.event_id.as_deref())
		.collect();

	let forward = body.forward
		|| (body.forward_if_no_messages && !has_messages(&services, room_id, &batch_ids).await);

	let build = |event: &batch_send::BatchSendEvent| PduBuilder {
		event_type: event.event_type.clone(),
		content: event.content.clone().into(),
		timestamp: Some(event.origin_server_ts),
		event_id_override: event.event_id.clone(),
		assume_sender_joined: true,
		..Default::default()
	};

	let mut event_ids: Vec<OwnedEventId> = Vec::with_capacity(body.events.len());
	let mut written: HashSet<OwnedEventId> = HashSet::new();
	let mut reactions: HashSet<(OwnedUserId, OwnedEventId, String)> = HashSet::new();
	if forward {
		for event in &body.events {
			if let Some(event_id) = existing_event_id(&services, room_id, event, &written).await?
			{
				event_ids.push(event_id);
				continue;
			}

			if !redaction_allowed(&services, room_id, event).await
				|| !thread_root_exists(&services, room_id, event).await
				|| is_duplicate_reaction(&services, room_id, event, &mut reactions).await
			{
				continue;
			}

			let event_id = match services
				.timeline
				.build_and_append_pdu_with(
					build(event),
					&event.sender,
					room_id,
					&state_lock,
					body.send_notification,
				)
				.await
			{
				| Ok(event_id) => event_id,
				| Err(e) if is_too_large(&e) => {
					warn!(%room_id, sender = %event.sender, "Skipping event too large to send: {e}");
					continue;
				},
				| Err(e) => return Err(e),
			};

			written.insert(event_id.clone());
			event_ids.push(event_id);
		}
	} else {
		// Each prepended event sorts before all previous ones, so insert the
		// newest first to keep the batch in chronological order.
		for event in body.events.iter().rev() {
			if let Some(event_id) = existing_event_id(&services, room_id, event, &written).await?
			{
				event_ids.push(event_id);
				continue;
			}

			if is_duplicate_reaction(&services, room_id, event, &mut reactions).await {
				continue;
			}

			let event_id = match services
				.timeline
				.build_and_prepend_pdu(build(event), &event.sender, room_id, &state_lock)
				.await
			{
				| Ok(event_id) => event_id,
				| Err(e) if is_too_large(&e) => {
					warn!(%room_id, sender = %event.sender, "Skipping event too large to send: {e}");
					continue;
				},
				| Err(e) => return Err(e),
			};

			written.insert(event_id.clone());
			event_ids.push(event_id);
		}

		event_ids.reverse();
	}

	drop(state_lock);

	// Backward batches are older than everything the user has already seen, so
	// only forward batches move the read markers.
	if forward
		&& let Some(user_id) = &body.mark_read_by
		&& let Some(last_event_id) = event_ids.last()
		&& let Err(e) =
			mark_read(&services, appservice_info, room_id, user_id, last_event_id).await
	{
		warn!(%room_id, %user_id, "Failed to mark batch as read: {e}");
	}

	Ok(batch_send::Response::new(event_ids))
}

async fn existing_event_id(
	services: &Services,
	room_id: &RoomId,
	event: &batch_send::BatchSendEvent,
	written: &HashSet<OwnedEventId>,
) -> Result<Option<OwnedEventId>> {
	let Some(event_id) = &event.event_id else {
		return Ok(None);
	};

	if written.contains(event_id) {
		return Ok(Some(event_id.clone()));
	}

	match services
		.timeline
		.get_non_outlier_pdu(event_id)
		.await
	{
		| Ok(pdu) if pdu.room_id() == room_id => Ok(Some(event_id.clone())),
		| Ok(_) =>
			Err!(Request(InvalidParam("Event {event_id} already exists in another room."))),
		| Err(e) if e.is_not_found() => Ok(None),
		| Err(e) => Err(e),
	}
}

async fn redaction_allowed(
	services: &Services,
	room_id: &RoomId,
	event: &batch_send::BatchSendEvent,
) -> bool {
	if event.event_type != TimelineEventType::RoomRedaction {
		return true;
	}

	let Some(redacts) = serde_json::from_str::<RoomRedactionEventContent>(event.content.get())
		.ok()
		.and_then(|content| content.redacts)
	else {
		return true;
	};

	let allowed = services
		.state_accessor
		.user_can_redact(&redacts, &event.sender, room_id, false)
		.await
		.unwrap_or(false);

	if !allowed {
		warn!(%room_id, sender = %event.sender, %redacts, "Skipping redaction the sender may not perform.");
	}

	allowed
}

async fn thread_root_exists(
	services: &Services,
	room_id: &RoomId,
	event: &batch_send::BatchSendEvent,
) -> bool {
	let Ok(ExtractRelatesTo { relates_to: Relation::Thread(thread) }) =
		serde_json::from_str(event.content.get())
	else {
		return true;
	};

	if services
		.timeline
		.get_pdu_id(&thread.event_id)
		.await
		.is_ok()
	{
		return true;
	}

	warn!(%room_id, sender = %event.sender, root = %thread.event_id, "Skipping thread message whose root is missing.");

	false
}

async fn is_duplicate_reaction(
	services: &Services,
	room_id: &RoomId,
	event: &batch_send::BatchSendEvent,
	reactions: &mut HashSet<(OwnedUserId, OwnedEventId, String)>,
) -> bool {
	if event.event_type != TimelineEventType::Reaction {
		return false;
	}

	let Ok(content) = serde_json::from_str::<ReactionEventContent>(event.content.get()) else {
		return false;
	};

	let target = content.relates_to.event_id;
	let key = content.relates_to.key;

	let duplicate = !reactions.insert((event.sender.clone(), target.clone(), key.clone()))
		|| services
			.pdu_metadata
			.event_has_relation(&target, Some(&event.sender), None, Some(&key))
			.await;

	if duplicate {
		warn!(%room_id, sender = %event.sender, %target, %key, "Skipping duplicate reaction.");
	}

	duplicate
}

fn is_too_large(error: &Error) -> bool { matches!(error.kind(), ErrorKind::TooLarge) }

async fn has_remote_servers(services: &Services, room_id: &RoomId) -> bool {
	services
		.state_cache
		.room_servers(room_id)
		.any(|server| {
			let remote = !services.globals.server_is_ours(server);
			async move { remote }
		})
		.await
}

/// Whether the room contains any non-state events, other than ones written by
/// an earlier attempt at this batch.
async fn has_messages(
	services: &Services,
	room_id: &RoomId,
	batch_ids: &HashSet<&EventId>,
) -> bool {
	let pdus = services.timeline.pdus_rev(None, room_id, None);
	pin_mut!(pdus);

	while let Some(item) = pdus.next().await {
		if let Ok((_, pdu)) = item
			&& pdu.state_key.is_none()
			&& !from_batch(&pdu, batch_ids)
		{
			return true;
		}
	}

	false
}

fn from_batch(pdu: &PduEvent, batch_ids: &HashSet<&EventId>) -> bool {
	batch_ids.contains(pdu.event_id())
		|| pdu
			.get_content::<ExtractRelatesTo>()
			.is_ok_and(|content| match content.relates_to {
				| Relation::Annotation(annotation) => batch_ids.contains(&*annotation.event_id),
				| _ => false,
			})
}

async fn mark_read(
	services: &Services,
	appservice_info: &RegistrationInfo,
	room_id: &RoomId,
	user_id: &UserId,
	event_id: &EventId,
) -> Result {
	if !services.globals.user_is_local(user_id) {
		return Err!(Request(InvalidParam("mark_read_by must be a local user.")));
	}

	if !services
		.appservice
		.can_act_as_user(appservice_info, user_id)
		.await
	{
		return Err!(Request(Exclusive("mark_read_by is not in the appservice's namespace.")));
	}

	if !services
		.state_cache
		.is_joined(user_id, room_id)
		.await
	{
		return Err!(Request(Forbidden("mark_read_by is not joined to the room.")));
	}

	let fully_read_event = FullyReadEvent {
		content: FullyReadEventContent { event_id: event_id.to_owned() },
	};

	services
		.account_data
		.update(
			Some(room_id),
			user_id,
			RoomAccountDataEventType::FullyRead,
			&serde_json::to_value(fully_read_event)?,
		)
		.await
		.map_err(|e| err!(Database("Failed to set fully read marker: {e}")))?;

	let receipt_content = BTreeMap::from_iter([(
		event_id.to_owned(),
		BTreeMap::from_iter([(
			ReceiptType::Read,
			BTreeMap::from_iter([(user_id.to_owned(), Receipt {
				ts: Some(MilliSecondsSinceUnixEpoch::now()),
				thread: ReceiptThread::Unthreaded,
			})]),
		)]),
	)]);

	services
		.read_receipt
		.readreceipt_update(user_id, room_id, &ReceiptEvent {
			content: ReceiptEventContent(receipt_content),
			room_id: room_id.to_owned(),
		})
		.await;

	services
		.pusher
		.reset_notification_counts_for_thread(user_id, room_id, None, &ReceiptThread::Unthreaded)
		.await;

	Ok(())
}
