use ruma::{OwnedRoomId, OwnedServerName};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn server_in_room(
	&self,
	server: OwnedServerName,
	room_id: OwnedRoomId,
) -> Result {
	let query = self
		.services
		.state_cache
		.server_in_room(&server, &room_id);

	self.write_timed_query(query).await
}
