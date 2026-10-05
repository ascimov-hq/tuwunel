use futures::TryStreamExt;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn appservice_all(&self) -> Result {
	let query = self
		.services
		.appservice
		.iter_db_ids()
		.try_collect::<Vec<_>>();

	self.write_timed_query_try(query).await
}
