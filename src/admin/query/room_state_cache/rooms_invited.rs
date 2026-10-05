use futures::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn rooms_invited(&self, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.state_cache
		.rooms_invited_state(&user_id)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
