use ruma::{OwnedDeviceId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_device_metadata(
	&self,
	user_id: OwnedUserId,
	device_id: OwnedDeviceId,
) -> Result {
	let query = self
		.services
		.users
		.get_device_metadata(&user_id, &device_id);

	self.write_timed_query(query).await
}
