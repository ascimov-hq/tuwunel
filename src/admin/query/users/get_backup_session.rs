use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_backup_session(
	&self,
	user_id: OwnedUserId,
	version: String,
	room_id: OwnedRoomId,
	session_id: String,
) -> Result {
	let query = self
		.services
		.key_backups
		.get_session(&user_id, &version, &room_id, &session_id);

	self.write_timed_query(query).await
}
