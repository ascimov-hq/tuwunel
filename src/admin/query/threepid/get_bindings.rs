use futures::stream::StreamExt;
use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_bindings(&self, user_id: OwnedUserId) -> Result {
	let query = self
		.services
		.threepid
		.get_bindings(&user_id)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
