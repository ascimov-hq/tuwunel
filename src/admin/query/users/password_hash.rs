use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn password_hash(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(self.services.users.password_hash(&user_id))
		.await
}
