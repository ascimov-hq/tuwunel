use std::collections::BTreeSet;

use axum::extract::State;
use futures::{StreamExt, future::ready};
use ruma::{
	EventId, MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedTransactionId, RoomId, UserId,
	events::{TimelineEventType, receipt::ReceiptThread},
};
use serde_json::from_str;
use tuwunel_core::{
	Err, Error, PduCount, Result, debug_info, err,
	matrix::{
		Event,
		pdu::{PduBuilder, SUPPRESS_NOTIFICATIONS},
		room_version,
	},
	utils::{result::LogErr, stream::TryIgnore},
	warn,
};
use tuwunel_service::{
	Services, appservice::RegistrationInfo, rooms::read_receipt::PrivateRead,
};

use self::endpoint::{BatchEvent, Request, Response};
use crate::Ruma;

/// Events scanned from the end of a room to decide whether it already has
/// messages (`forward_if_no_messages`).
const MESSAGES_SCAN_LIMIT: usize = 1000;

/// # `POST /_matrix/client/unstable/com.beeper.backfill/rooms/{roomId}/batch_send`
///
/// Beeper batch sending (`com.beeper.batch_sending`), forward only: appends
/// the given message events at the end of the room in order, under one hold
/// of the room's state lock, and returns the event IDs this server assigned,
/// in the same order. Callable only by appservices (the endpoint's
/// `AppserviceToken` scheme), only into rooms belonging to the caller (see
/// [`check_room_owned`]), for senders in their namespace or covered by a
/// URL-less double-puppeting registration, which must be joined to the room.
/// The server does not store or honour client-chosen event IDs.
///
/// - `origin_server_ts` of each event is kept (appservice timestamp
///   massaging).
/// - `send_notification: false` appends without push-rule evaluation,
///   notification counts or pushes.
/// - `mark_read_by` moves that local user's private read marker to the last
///   event of the batch.
/// - `forward: false` is accepted only together with `forward_if_no_messages`
///   in a room without messages, and is then sent forward as well.
/// - An optional `txn_id` (body or query) makes retries idempotent: a repeated
///   `txn_id` for the same room returns the stored event IDs.
pub(crate) async fn batch_send_route(
	State(services): State<crate::State>,
	body: Ruma<Request>,
) -> Result<Response> {
	if !services.config.beeper_batch_send {
		return Err!(Request(Unrecognized("Batch sending is not enabled on this server.")));
	}

	let Some(appservice) = body.appservice_info.as_ref() else {
		return Err!(Request(Forbidden("Batch sending is restricted to appservices.")));
	};

	let room_id = &body.room_id;
	let batch = &body.batch;
	if batch.events.is_empty() {
		return Ok(Response { event_ids: Vec::new() });
	}

	if !services.metadata.exists(room_id).await {
		return Err!(Request(NotFound("Room does not exist on this server.")));
	}

	// Scoped per room so ids cannot collide with the sender's /send txn ids.
	let txn_id = body
		.txn_id
		.as_deref()
		.map(|txn_id| format!("com.beeper.batch_send:{room_id}:{txn_id}"))
		.map(OwnedTransactionId::from);

	// Held across the txn lookup, the checks and all appends, so a retried
	// request waits for the original and then gets its stored result.
	let state_lock = services.state.mutex.lock(room_id).await;

	if let Some(txn_id) = txn_id.as_deref()
		&& let Ok(response) = services
			.transaction_ids
			.existing_txnid(&appservice.sender, None, txn_id)
			.await
	{
		let event_ids: Vec<OwnedEventId> = serde_json::from_slice(&response)
			.map_err(|e| err!(Database("Invalid batch send txn_id data: {e}")))?;

		return Ok(Response { event_ids });
	}

	check_room_owned(&services, appservice, room_id).await?;

	check_events(&services, appservice, room_id, &batch.events).await?;

	if let Some(user_id) = batch.mark_read_by.as_deref() {
		check_mark_read_by(&services, appservice, room_id, user_id).await?;
	}

	// A backward batch would have to go before existing history; only a room
	// without messages may take one, and it is then appended forward.
	let forward = batch.forward
		|| (batch.forward_if_no_messages && !room_has_messages(&services, room_id).await);

	if !forward {
		return Err!(Request(InvalidParam(
			"Only forward batch sending is supported; backward batches are accepted only into \
			 rooms without messages (forward_if_no_messages)."
		)));
	}

	let append = async {
		let mut event_ids = Vec::with_capacity(batch.events.len());
		for event in &batch.events {
			let pdu_builder = PduBuilder {
				event_type: event.event_type.clone(),
				content: from_str(event.content.get())
					.map_err(|e| err!(Request(BadJson("Invalid event content: {e}"))))?,
				timestamp: event.origin_server_ts,
				..Default::default()
			};

			let event_id = services
				.timeline
				.build_and_append_pdu(pdu_builder, &event.sender, room_id, &state_lock)
				.await
				.inspect_err(|e| {
					warn!(
						%room_id,
						appended = event_ids.len(),
						total = batch.events.len(),
						"Batch send stopped: {e}"
					);
				})?;

			event_ids.push(event_id);
		}

		Ok::<_, Error>(event_ids)
	};

	let event_ids = SUPPRESS_NOTIFICATIONS
		.scope(!batch.send_notification, append)
		.await?;

	if let Some(txn_id) = txn_id.as_deref() {
		let response = serde_json::to_vec(&event_ids)?;
		services
			.transaction_ids
			.add_txnid(&appservice.sender, None, txn_id, &response);
	}

	drop(state_lock);

	if let (Some(user_id), Some(event_id)) = (batch.mark_read_by.as_deref(), event_ids.last()) {
		mark_read(&services, room_id, user_id, event_id)
			.await
			.log_err()
			.ok();
	}

	debug_info!(
		%room_id,
		events = event_ids.len(),
		notify = batch.send_notification,
		"Batch sent"
	);

	Ok(Response { event_ids })
}

/// Only rooms belonging to the calling appservice take batches: its bot
/// (`sender_localpart`) is joined, or one of the room's creators is the bot or
/// a user in its exclusive user namespace. Without this an appservice could
/// batch-send as double-puppeted users (see [`is_double_puppet`]) into rooms
/// of other bridges or of plain users.
async fn check_room_owned(
	services: &Services,
	appservice: &RegistrationInfo,
	room_id: &RoomId,
) -> Result {
	if services
		.state_cache
		.is_joined(&appservice.sender, room_id)
		.await
	{
		return Ok(());
	}

	let created_by_appservice = match services.state_accessor.get_create(room_id).await {
		| Err(_) => false,
		| Ok(create) => {
			let room_version = create.room_version()?;
			let rules = room_version::rules(&room_version)?;
			create
				.creators(&rules.authorization)?
				.any(|creator| appservice.is_exclusive_user_match(&creator))
		},
	};

	if !created_by_appservice {
		return Err!(Request(Forbidden(
			"The room does not belong to this appservice: its bot is not joined and it did not \
			 create the room."
		)));
	}

	Ok(())
}

/// Rejects the whole batch before anything is appended when an event could
/// not be sent: state events and redactions are not batched, every sender
/// must be in the appservice's namespace (or double-puppeted, see
/// [`is_double_puppet`]) and joined to the room.
async fn check_events(
	services: &Services,
	appservice: &RegistrationInfo,
	room_id: &RoomId,
	events: &[BatchEvent],
) -> Result {
	for event in events {
		if event.state_key.is_some() {
			return Err!(Request(InvalidParam("State events cannot be batch sent.")));
		}

		if event.event_type == TimelineEventType::RoomRedaction {
			return Err!(Request(InvalidParam("Redactions cannot be batch sent.")));
		}

		if event.event_type == TimelineEventType::RoomEncrypted
			&& !services.config.allow_encryption
		{
			return Err!(Request(Forbidden("Encryption has been disabled")));
		}
	}

	let senders: BTreeSet<&UserId> = events.iter().map(|event| &*event.sender).collect();

	for sender in senders {
		if !appservice.is_user_match(sender) && !is_double_puppet(services, sender).await {
			return Err!(Request(Exclusive("Sender {sender} is not in the namespace.")));
		}

		if !services.state_cache.is_joined(sender, room_id).await {
			return Err!(Request(Forbidden("Sender {sender} is not joined to the room.")));
		}
	}

	Ok(())
}

/// Bridges send a user's own messages as that user (double puppeting) through
/// a URL-less appservice registration whose token they hold; mautrix batches
/// such events together with the ghosts' under the bridge's own token. Users
/// claimed exclusively by any appservice (its bot or ghosts, e.g. a URL-less
/// maintenance appservice's own user) are never double puppets.
async fn is_double_puppet(services: &Services, user_id: &UserId) -> bool {
	if !services.globals.user_is_local(user_id) {
		return false;
	}

	let appservices = services.appservice.read().await;
	if appservices
		.values()
		.any(|info| info.is_exclusive_user_match(user_id))
	{
		return false;
	}

	appservices
		.values()
		.filter(|info| {
			// Same notion of "no URL" as the appservice sender (`url: null`,
			// empty, or the literal "null").
			info.registration
				.url
				.as_deref()
				.is_none_or(|url| url.is_empty() || url == "null")
		})
		.any(|info| info.is_user_match(user_id))
}

/// `mark_read_by` must be a local member of the room the calling appservice
/// may act for (its namespace or a double puppet), so it cannot move the read
/// marker and notification counts of arbitrary users.
async fn check_mark_read_by(
	services: &Services,
	appservice: &RegistrationInfo,
	room_id: &RoomId,
	user_id: &UserId,
) -> Result {
	if !appservice.is_user_match(user_id) && !is_double_puppet(services, user_id).await {
		return Err!(Request(Exclusive("mark_read_by {user_id} is not in the namespace.")));
	}

	if !services.globals.user_is_local(user_id)
		|| !services.state_cache.is_joined(user_id, room_id).await
	{
		return Err!(Request(InvalidParam("mark_read_by must be a local member of the room.")));
	}

	Ok(())
}

/// Whether the room has any non-state event among its most recent events.
async fn room_has_messages(services: &Services, room_id: &RoomId) -> bool {
	services
		.timeline
		.pdus_rev(None, room_id, None)
		.ignore_err()
		.take(MESSAGES_SCAN_LIMIT)
		.any(|(_, pdu)| ready(pdu.state_key().is_none()))
		.await
}

/// Moves `user_id`'s private read marker to `event_id` and clears the room's
/// notification counts when it advanced, as a private read receipt would.
async fn mark_read(
	services: &Services,
	room_id: &RoomId,
	user_id: &UserId,
	event_id: &EventId,
) -> Result {
	let PduCount::Normal(count) = services.timeline.get_pdu_count(event_id).await? else {
		return Ok(());
	};

	let advanced = services
		.read_receipt
		.private_read_set(PrivateRead {
			room_id,
			user_id,
			count,
			ts: MilliSecondsSinceUnixEpoch::now(),
			thread: &ReceiptThread::Unthreaded,
			announce: true,
		})
		.await;

	if advanced
		&& services
			.pusher
			.reset_notification_counts_for_thread(user_id, room_id, &ReceiptThread::Unthreaded)
			.await
	{
		services.sending.refresh_push_badge(user_id).await?;
	}

	Ok(())
}

/// Endpoint definition. Ruma has no type for this Beeper extension and its
/// request/response macros need crate features this crate does not declare,
/// so the traits are implemented by hand.
mod endpoint {
	use bytes::BufMut;
	use ruma::{
		MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedRoomId, OwnedTransactionId, OwnedUserId,
		api::{
			IncomingRequest, OutgoingResponse,
			auth_scheme::AppserviceToken,
			error::{Error as EndpointError, FromHttpRequestError, IntoHttpError},
			metadata,
		},
		events::TimelineEventType,
		serde::json_to_buf,
	};
	use serde::{Deserialize, Serialize};
	use serde_json::value::RawValue as RawJsonValue;

	metadata! {
		method: POST,
		rate_limited: false,
		authentication: AppserviceToken,
		history: {
			unstable => "/_matrix/client/unstable/com.beeper.backfill/rooms/{room_id}/batch_send",
		}
	}

	#[derive(Debug)]
	pub(crate) struct Request {
		pub(crate) room_id: OwnedRoomId,

		/// Idempotency key: from the body, else from the `txn_id` query
		/// parameter.
		pub(crate) txn_id: Option<OwnedTransactionId>,

		pub(crate) batch: RequestBody,
	}

	/// Body as sent by mautrix-go (`mautrix.ReqBeeperBatchSend`).
	#[derive(Debug, Deserialize)]
	pub(crate) struct RequestBody {
		#[serde(default)]
		pub(crate) forward: bool,

		#[serde(default)]
		pub(crate) forward_if_no_messages: bool,

		#[serde(default)]
		pub(crate) send_notification: bool,

		#[serde(default)]
		pub(crate) mark_read_by: Option<OwnedUserId>,

		pub(crate) events: Vec<BatchEvent>,

		#[serde(default)]
		txn_id: Option<OwnedTransactionId>,
	}

	/// One event of the batch; `event_id`, `room_id` and `unsigned` sent by
	/// the client are ignored.
	#[derive(Debug, Deserialize)]
	pub(crate) struct BatchEvent {
		pub(crate) sender: OwnedUserId,

		#[serde(rename = "type")]
		pub(crate) event_type: TimelineEventType,

		pub(crate) content: Box<RawJsonValue>,

		#[serde(default)]
		pub(crate) origin_server_ts: Option<MilliSecondsSinceUnixEpoch>,

		#[serde(default)]
		pub(crate) state_key: Option<String>,
	}

	#[derive(Debug, Serialize)]
	pub(crate) struct Response {
		pub(crate) event_ids: Vec<OwnedEventId>,
	}

	impl IncomingRequest for Request {
		type EndpointError = EndpointError;
		type OutgoingResponse = Response;

		fn try_from_http_request<B, S>(
			request: http::Request<B>,
			path_args: &[S],
		) -> Result<Self, FromHttpRequestError>
		where
			B: AsRef<[u8]>,
			S: AsRef<str>,
		{
			Self::check_request_method(request.method())?;

			let room_id = path_args
				.first()
				.map(AsRef::as_ref)
				.unwrap_or_default();

			let room_id = OwnedRoomId::try_from(room_id)?;

			let mut batch: RequestBody = serde_json::from_slice(request.body().as_ref())?;

			let query_txn_id = request
				.uri()
				.query()
				.map(str::as_bytes)
				.map(url::form_urlencoded::parse)
				.into_iter()
				.flatten()
				.find_map(|(key, value)| (key == "txn_id").then(|| value.into_owned()))
				.map(OwnedTransactionId::from);

			let txn_id = batch.txn_id.take().or(query_txn_id);

			Ok(Self { room_id, txn_id, batch })
		}
	}

	impl OutgoingResponse for Response {
		fn try_into_http_response<T: Default + BufMut>(
			self,
		) -> Result<http::Response<T>, IntoHttpError> {
			let body = json_to_buf(&self)?;

			Ok(http::Response::builder()
				.header(http::header::CONTENT_TYPE, "application/json")
				.body(body)?)
		}
	}
}
