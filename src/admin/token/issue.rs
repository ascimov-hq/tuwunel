use tuwunel_core::{Result, utils};
use tuwunel_service::registration_tokens::TokenExpires;

use crate::admin_command;

#[admin_command]
pub(super) async fn issue(
	&self,
	max_uses: Option<u64>,
	max_age: Option<String>,
	once: bool,
) -> Result {
	let expires = TokenExpires {
		max_uses: max_uses.or_else(|| once.then_some(1)),
		max_age: max_age
			.map(|max_age| {
				let duration = utils::time::parse_duration(&max_age)?;
				utils::time::timepoint_from_now(duration)
			})
			.transpose()?,
	};

	let (token, info) = self
		.services
		.registration_tokens
		.create_token(None, None, expires)
		.await?;

	write!(self, "New registration token issued: `{token}` - {info}").await
}
