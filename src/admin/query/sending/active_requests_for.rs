use futures::StreamExt;
use ruma::{OwnedServerName, OwnedUserId};
use tuwunel_core::Result;

use super::parse_destination;
use crate::admin_command;

#[admin_command]
pub(super) async fn sending_active_requests_for(
	&self,
	appservice_id: Option<String>,
	server_name: Option<OwnedServerName>,
	user_id: Option<OwnedUserId>,
	push_key: Option<String>,
) -> Result {
	let destination = parse_destination(appservice_id, server_name, user_id, push_key)?;

	let query = self
		.services
		.sending
		.db
		.active_requests_for(&destination)
		.collect::<Vec<_>>();

	self.write_timed_query(query).await
}
