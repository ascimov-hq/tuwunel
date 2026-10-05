use ruma::OwnedServerName;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn sending_get_latest_edu_count(&self, server_name: OwnedServerName) -> Result {
	let query = self
		.services
		.sending
		.db
		.get_latest_educount(&server_name);

	self.write_timed_query(query).await
}
