mod read_markers;
mod receipt;

use ruma::{EventId, MilliSecondsSinceUnixEpoch, RoomId, UserId, events::receipt::ReceiptThread};
use tuwunel_core::{Err, PduCount, Result, err};
use tuwunel_service::{Services, rooms::read_receipt::PrivateRead};

pub(crate) use self::{read_markers::set_read_marker_route, receipt::create_receipt_route};

/// Resolves `event` to its timeline position and stores the private read
/// marker for `thread` there.
///
/// Returns whether the marker advanced. A backfilled event carries no forward
/// position, so it is rejected rather than stored.
async fn set_private_marker(
	services: &Services,
	room_id: &RoomId,
	user_id: &UserId,
	event: &EventId,
	thread: &ReceiptThread,
) -> Result<bool> {
	let count = services
		.timeline
		.get_pdu_count(event)
		.await
		.map_err(|_| err!(Request(NotFound("Event not found."))))?;

	let PduCount::Normal(count) = count else {
		return Err!(Request(InvalidParam(
			"Event is a backfilled PDU and cannot be marked as read."
		)));
	};

	let advanced = services
		.read_receipt
		.private_read_set(PrivateRead {
			room_id,
			user_id,
			count,
			ts: MilliSecondsSinceUnixEpoch::now(),
			thread,
			announce: true,
		})
		.await;

	Ok(advanced)
}
