use ruma::OwnedRoomAliasId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn resolve_alias(&self, alias: OwnedRoomAliasId) -> Result {
	self.write_timed_query(self.services.alias.resolve_alias(&alias))
		.await
}
