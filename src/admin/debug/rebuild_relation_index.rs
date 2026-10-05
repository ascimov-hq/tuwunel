use tuwunel_core::Result;

use crate::admin_command;

#[admin_command]
pub(super) async fn rebuild_relation_index(&self) -> Result {
	self.services
		.pdu_metadata
		.rebuild_typed_relations()
		.await?;

	self.write_str("Rebuilt the relatesto_typed relation index.")
		.await
}
