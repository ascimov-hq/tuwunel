use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn globals_database_version(&self) -> Result {
	self.write_timed_query(self.services.globals.db.database_version())
		.await
}
