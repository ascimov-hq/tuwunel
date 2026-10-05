use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_shared_rooms(&self, user_a: OwnedUserId, user_b: OwnedUserId) -> Result {
	let query = self
		.services
		.state_cache
		.get_shared_rooms(&user_a, &user_b)
		.map(ToOwned::to_owned)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
