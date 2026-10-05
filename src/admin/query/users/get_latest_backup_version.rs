use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_latest_backup_version(&self, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.key_backups
		.get_latest_backup_version(&user_id);

	self.write_timed_query(query).await
}
