use futures::StreamExt;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn all_local_aliases(&self) -> Result {
	let query = self
		.services
		.alias
		.all_local_aliases()
		.map(|(room_id, alias)| (room_id.to_owned(), alias.to_owned()))
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
