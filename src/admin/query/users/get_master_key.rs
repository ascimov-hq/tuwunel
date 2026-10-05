use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_master_key(&self, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.users
		.get_master_key(None, &user_id, &|_| true);

	self.write_timed_query(query).await
}
