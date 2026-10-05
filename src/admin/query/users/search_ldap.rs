use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn search_ldap(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(self.services.users.search_ldap(&user_id))
		.await
}
