use ruma::OwnedServerName;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn globals_signing_keys_for(&self, origin: OwnedServerName) -> Result {
	self.write_timed_query(self.services.server_keys.verify_keys_for(&origin))
		.await
}
