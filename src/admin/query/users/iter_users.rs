use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::{Result, utils::ReadyExt};

use crate::admin_command;

#[admin_command]
pub(super) async fn iter_users(&self, historical: bool) -> Result {
	let query = self
		.services
		.users
		.stream()
		.ready_filter(|user_id| !historical || user_id.is_historical())
		.map(Into::into)
		.collect::<Vec<OwnedUserId>>();

	self.write_timed_query(query).await
}
