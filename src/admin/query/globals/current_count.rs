use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn globals_current_count(&self) -> Result {
	self.write_timed_query(async { self.services.globals.current_count() })
		.await
}
