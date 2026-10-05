use futures::StreamExt;
use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn changes_since(
	&self,
	user_id: OwnedUserId,
	since: u64,
	room_id: Option<OwnedRoomId>,
) -> Result {
	let query = self
		.services
		.account_data
		.changes_since(room_id.as_deref(), &user_id, since, None)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
