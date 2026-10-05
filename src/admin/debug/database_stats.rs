use futures::TryStreamExt;
use tuwunel_core::{
	Result,
	utils::{stream::IterStream, string::EMPTY},
};

use crate::admin_command;

#[admin_command]
pub(super) async fn database_stats(
	&self,
	property: Option<String>,
	map: Option<String>,
) -> Result {
	let map_name = map.as_ref().map_or(EMPTY, String::as_str);
	let property = property.unwrap_or_else(|| "rocksdb.stats".to_owned());
	self.services
		.db
		.iter()
		.filter(|&(&name, _)| map_name.is_empty() || map_name == name)
		.try_stream()
		.try_for_each(|(&name, map)| {
			let res = map
				.property(&property)
				.unwrap_or_else(|e| format!("invalid property: {e}"));

			writeln!(self, "##### {name}:\n```\n{}\n```", res.trim())
		})
		.await
}
