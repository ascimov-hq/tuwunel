use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_all_backups(&self, user_id: OwnedUserId, version: String) -> Result {
	let query = self
		.services
		.key_backups
		.get_all(&user_id, &version);

	self.write_timed_query(query).await
}
