use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_room_backups(
	&self,
	user_id: OwnedUserId,
	version: String,
	room_id: OwnedRoomId,
) -> Result {
	let query = self
		.services
		.key_backups
		.get_room(&user_id, &version, &room_id);

	self.write_timed_query(query).await
}
