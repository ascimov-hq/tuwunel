use tuwunel_core::Result;
use tuwunel_service::threepid::canonicalize_email;

use crate::admin_command;

#[admin_command]
pub(super) async fn address_in_use(&self, address: String) -> Result {
	let email_canon = canonicalize_email(&address)?;

	let query = self
		.services
		.threepid
		.address_in_use(&email_canon);

	self.write_timed_query(query).await
}
