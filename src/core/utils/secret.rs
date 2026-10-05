#[cfg(test)]
mod tests;

use std::{fs::read_to_string, path::Path};

use crate::{error, smallstr::SmallString};

pub type Secret = SmallString<[u8; 64]>;

/// Resolves a secret configured either in `file` or inline.
///
/// The file is preferred, and its contents are trimmed because an editor or a
/// systemd credential commonly leaves a trailing newline which the peer holding
/// the other copy of the secret does not have. An inline value is taken as
/// written. An unset or unreadable file falls back to `inline`; one which is
/// present but blank resolves to nothing instead. `name` labels the read
/// failure in the log.
/// Whether a secret is configured at all.
///
/// Answered without opening the file, so an unauthenticated caller cannot drive
/// filesystem work. A configured file which turns out to be unreadable or blank
/// still counts as set here, and only [`resolve`] discovers otherwise.
#[must_use]
pub fn is_set(file: Option<&Path>, inline: Option<&str>) -> bool {
	file.is_some() || inline.is_some_and(|inline| !inline.is_empty())
}

#[must_use]
pub fn resolve(file: Option<&Path>, inline: Option<&str>, name: &str) -> Option<Secret> {
	let from_file = file.and_then(|path| {
		read_to_string(path)
			.inspect_err(|e| error!(%e, %name, "Failed to read secret file"))
			.ok()
	});

	from_file
		.as_deref()
		.map(str::trim)
		.or(inline)
		.filter(|secret| !secret.is_empty())
		.map(Secret::from)
}
