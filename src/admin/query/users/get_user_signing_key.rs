use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_user_signing_key(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(self.services.users.get_user_signing_key(&user_id))
		.await
}
