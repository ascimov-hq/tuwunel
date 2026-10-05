use futures::StreamExt;
use ruma::OwnedRoomId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn room_user_once_joined(&self, room_id: OwnedRoomId) -> Result {
	let query = self
		.services
		.state_cache
		.room_useroncejoined(&room_id)
		.map(ToOwned::to_owned)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
