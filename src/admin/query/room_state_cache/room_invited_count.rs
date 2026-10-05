use ruma::OwnedRoomId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn room_invited_count(&self, room_id: OwnedRoomId) -> Result {
	let query = self
		.services
		.state_cache
		.room_invited_count(&room_id);

	self.write_timed_query(query).await
}
