use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn appservice_get_registration(&self, appservice_id: String) -> Result {
	let query = self
		.services
		.appservice
		.get_registration(&appservice_id);

	self.write_timed_query(query).await
}
