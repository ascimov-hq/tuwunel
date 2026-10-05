use std::collections::BTreeSet;

use ruma::{OwnedUserId, ServerName, UserId, api::appservice::Registration};
use tuwunel_core::Result;

use super::NamespaceRegex;

/// Appservice registration combined with its compiled regular expressions.
#[derive(Clone, Debug)]
pub struct RegistrationInfo {
	pub aliases: NamespaceRegex,
	pub users: NamespaceRegex,
	pub rooms: NamespaceRegex,
	pub sender: OwnedUserId,
	pub registration: Registration,
}

impl RegistrationInfo {
	pub fn new(registration: Registration, server_name: &ServerName) -> Result<Self> {
		Ok(Self {
			aliases: NamespaceRegex::new(false, registration.namespaces.aliases.iter())?,
			users: NamespaceRegex::new(false, registration.namespaces.users.iter())?,
			rooms: NamespaceRegex::new(true, registration.namespaces.rooms.iter())?,
			sender: OwnedUserId::from_parts(
				'@',
				registration.sender_localpart.as_ref(),
				server_name.as_str().into(),
			)?,

			registration,
		})
	}

	/// MSC3905: the `users` regex matches local users only.
	#[inline]
	#[must_use]
	pub fn is_user_match(&self, user_id: &UserId) -> bool {
		user_id == self.sender
			|| (self.users.is_match(user_id.as_str())
				&& user_id.server_name() == self.sender.server_name())
	}

	/// MSC3905: the `users` regex matches local users only.
	#[inline]
	#[must_use]
	pub fn is_exclusive_user_match(&self, user_id: &UserId) -> bool {
		user_id == self.sender
			|| (self.users.is_exclusive_match(user_id.as_str())
				&& user_id.server_name() == self.sender.server_name())
	}

	/// MSC3983/MSC3984 key proxies: the registration opted in (its ID is
	/// listed in the global `appservice_key_proxy`) and the user is its sender
	/// or in its exclusive user namespace, as Synapse requires.
	#[inline]
	#[must_use]
	pub fn is_key_proxy_match(&self, opted_in: &BTreeSet<String>, user_id: &UserId) -> bool {
		opted_in.contains(&self.registration.id) && self.is_exclusive_user_match(user_id)
	}
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeSet;

	use ruma::{api::appservice::Registration, server_name, user_id};

	use super::RegistrationInfo;

	fn bridge() -> RegistrationInfo {
		let registration: Registration = serde_yaml::from_str(
			r"
id: whatsapp
url: http://bridge:29318
as_token: as_token
hs_token: hs_token
sender_localpart: whatsappbot
rate_limited: false
namespaces:
  users:
  - regex: '^@whatsapp_[0-9]+:example\.com$'
    exclusive: true
  - regex: '^@shared_.*:example\.com$'
    exclusive: false
",
		)
		.expect("valid registration");

		RegistrationInfo::new(registration, server_name!("example.com"))
			.expect("valid namespaces")
	}

	fn ids(ids: &[&str]) -> BTreeSet<String> { ids.iter().map(ToString::to_string).collect() }

	#[test]
	fn key_proxy_needs_opt_in() {
		let info = bridge();
		let ghost = user_id!("@whatsapp_15550000001:example.com");

		assert!(info.is_exclusive_user_match(ghost));
		assert!(!info.is_key_proxy_match(&ids(&[]), ghost));
		assert!(!info.is_key_proxy_match(&ids(&["telegram"]), ghost));
		assert!(info.is_key_proxy_match(&ids(&["telegram", "whatsapp"]), ghost));
	}

	#[test]
	fn key_proxy_only_exclusive_local_users_and_sender() {
		let info = bridge();
		let opted_in = ids(&["whatsapp"]);

		assert!(info.is_key_proxy_match(&opted_in, user_id!("@whatsappbot:example.com")));
		assert!(info.is_key_proxy_match(&opted_in, user_id!("@whatsapp_1:example.com")));

		// non-exclusive namespace: matched by is_user_match, but not proxied
		let shared = user_id!("@shared_1:example.com");
		assert!(info.is_user_match(shared));
		assert!(!info.is_key_proxy_match(&opted_in, shared));

		assert!(!info.is_key_proxy_match(&opted_in, user_id!("@whatsapp_1:other.example")));
		assert!(!info.is_key_proxy_match(&opted_in, user_id!("@alice:example.com")));
	}
}
