use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_left_count(&self, room_id: OwnedRoomId, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.state_cache
		.get_left_count(&room_id, &user_id);

	self.write_timed_query(query).await
}
