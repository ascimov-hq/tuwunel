use futures::stream::StreamExt;
use ruma::{OwnedDeviceId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_to_device_events(
	&self,
	user_id: OwnedUserId,
	device_id: OwnedDeviceId,
) -> Result {
	let query = self
		.services
		.users
		.get_to_device_events(&user_id, &device_id, None, None)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
