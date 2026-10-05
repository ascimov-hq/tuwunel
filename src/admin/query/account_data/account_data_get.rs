use ruma::{OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;
use tuwunel_database::Deserialized;

use crate::admin_command;

#[admin_command]
pub(super) async fn account_data_get(
	&self,
	user_id: OwnedUserId,
	kind: String,
	room_id: Option<OwnedRoomId>,
) -> Result {
	let query = async {
		self.services
			.account_data
			.get_raw(room_id.as_deref(), &user_id, &kind)
			.await
			.deserialized::<serde_json::Value>()
	};

	self.write_timed_query_try(query).await
}
