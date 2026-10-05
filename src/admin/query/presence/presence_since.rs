use futures::StreamExt;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn presence_presence_since(&self, since: u64, to: Option<u64>) -> Result {
	let query = self
		.services
		.presence
		.presence_since(since, to)
		.map(|(user_id, count, bytes)| (user_id.to_owned(), count, bytes.to_vec()))
		.collect::<Vec<(_, _, _)>>();

	self.write_timed_query(query).await
}
