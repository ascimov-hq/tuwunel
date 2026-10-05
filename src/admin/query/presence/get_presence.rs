use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn presence_get_presence(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(self.services.presence.get_presence(&user_id))
		.await
}
