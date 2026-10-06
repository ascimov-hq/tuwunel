use ruma::{OwnedEventId, RoomId, UserId, events::room::encrypted::Relation};
use tuwunel_core::{
	Result, implement,
	matrix::{
		event::Event,
		pdu::{PduBuilder, PduCount, PduId, RawPduId},
	},
	utils::result::LogErr,
	validated,
};

use super::{ExtractBody, ExtractRelatesTo, RoomMutexGuard};

/// Creates a new persisted data unit and inserts it before the start of the
/// room's timeline, for appservice batch sending (backward backfill).
///
/// The event is authorized against the current room state and references the
/// current forward extremities, but it is not added to the forward extremities
/// itself, nor does it update the room state, send push notifications, read
/// receipts, appservice transactions or federation traffic. Only message
/// events may be prepended.
#[implement(super::Service)]
#[tracing::instrument(
	name = "build_and_prepend",
	level = "debug",
	skip(self, state_lock),
	ret
)]
pub async fn build_and_prepend_pdu(
	&self,
	pdu_builder: PduBuilder,
	sender: &UserId,
	room_id: &RoomId,
	state_lock: &RoomMutexGuard,
) -> Result<OwnedEventId> {
	let (pdu, pdu_json) = self
		.create_hash_and_sign_event(pdu_builder, sender, room_id, state_lock)
		.await?;

	let shortroomid = self
		.services
		.short
		.get_shortroomid(room_id)
		.await?;

	let insert_lock = self.mutex_insert.lock(room_id).await;

	// Backfilled events are counted in ℤ−, so each new one sorts before every
	// event already in the timeline.
	let count = self.services.globals.next_count();
	let count: i64 = (*count).try_into()?;
	let count = PduCount::Backfilled(validated!(0 - count));
	let pdu_id: RawPduId = PduId { shortroomid, count }.into();

	self.prepend_backfill_pdu(
		&pdu_id,
		room_id,
		pdu.event_id(),
		u64::from(pdu.origin_server_ts),
		&pdu_json,
	);
	drop(insert_lock);

	if let Ok(content) = pdu.get_content::<ExtractBody>()
		&& let Some(body) = content.body
	{
		self.services
			.search
			.index_pdu(shortroomid, &pdu_id, &body);
	}

	// Relations to and from backfilled events aren't indexed by pdu_metadata
	// yet, but threads are.
	if let Ok(content) = pdu.get_content::<ExtractRelatesTo>()
		&& let Relation::Thread(thread) = content.relates_to
	{
		self.services
			.threads
			.add_to_thread(&thread.event_id, &pdu)
			.await
			.log_err()
			.ok();
	}

	Ok(pdu.event_id().to_owned())
}
