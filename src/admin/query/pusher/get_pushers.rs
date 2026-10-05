use ruma::OwnedUserId;
use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn get_pushers(&self, user_id: OwnedUserId) -> Result {
	self.write_timed_query(self.services.pusher.get_pushers(&user_id))
		.await
}
