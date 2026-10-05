use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn count_users(&self) -> Result {
	self.write_timed_query(self.services.users.count())
		.await
}
