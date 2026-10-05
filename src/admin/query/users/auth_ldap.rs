use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn auth_ldap(&self, user_dn: String, password: String) -> Result {
	self.write_timed_query(self.services.users.auth_ldap(&user_dn, &password))
		.await
}
