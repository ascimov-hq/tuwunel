use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn list_devices(&self, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.users
		.all_device_ids(&user_id)
		.map(ToOwned::to_owned)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
