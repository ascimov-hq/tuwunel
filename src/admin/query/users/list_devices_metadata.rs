use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn list_devices_metadata(&self, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.users
		.all_devices_metadata(&user_id)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
