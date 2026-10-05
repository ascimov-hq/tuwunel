use futures::StreamExt;
use ruma::OwnedRoomId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn local_aliases_for_room(&self, room_id: OwnedRoomId) -> Result {
	let query = self
		.services
		.alias
		.local_aliases_for_room(&room_id)
		.map(ToOwned::to_owned)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
