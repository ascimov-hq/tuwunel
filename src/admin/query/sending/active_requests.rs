use futures::StreamExt;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn sending_active_requests(&self) -> Result {
	let query = self
		.services
		.sending
		.db
		.active_requests()
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
