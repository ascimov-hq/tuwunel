//! HTML text escaping utilities.
//!
//! The helper replaces six markup-sensitive ASCII characters with entity
//! references. Existing entity references are escaped again because ampersands
//! are processed first.

/// Escapes HTML-sensitive ASCII characters in a string.
///
/// Ampersands, angle brackets, both quote characters, and backticks are
/// replaced with entity references. Existing entity references are escaped
/// again.
#[must_use]
pub fn escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
		.replace('`', "&#x60;")
}
