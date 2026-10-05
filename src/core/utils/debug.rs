//! Bounded and redacted debug-formatting utilities.
//!
//! The wrappers cap debug output from slices and strings before values enter
//! tracing fields. The exported macro reports optional presence without
//! exposing contents.

use std::fmt;

/// Wraps a slice for length-limited `Debug` output.
///
/// Slices at or below `max_len` keep ordinary slice formatting. Longer slices
/// show the first `max_len` elements followed by a quoted `"..."` list entry.
pub struct TruncatedSlice<'a, T> {
	inner: &'a [T],
	max_len: usize,
}

/// Wraps a UTF-8 string for threshold-limited `Debug` output.
///
/// Strings no longer than `max_len` bytes keep ordinary quoted formatting.
/// Longer strings end at the first scalar boundary at or after `max_len`, then
/// append `...` outside the closing quote. Formatting panics if `max_len` lies
/// within the final multibyte scalar because no later boundary exists.
pub struct TruncatedStr<'a> {
	inner: &'a str,
	max_len: usize,
}

/// Creates a tracing debug value that truncates a slice.
///
/// The returned value can be recorded directly in a structured tracing field.
/// At most `max_len` slice elements are formatted before the ellipsis marker.
pub fn slice_truncated<T: fmt::Debug>(
	slice: &[T],
	max_len: usize,
) -> tracing::field::DebugValue<TruncatedSlice<'_, T>> {
	tracing::field::debug(TruncatedSlice { inner: slice, max_len })
}

/// Creates a tracing debug value that truncates a string.
///
/// The returned value can be recorded directly in a structured tracing field.
/// Truncation uses a byte threshold and ends at the first following UTF-8
/// boundary. Formatting panics if the threshold lies within the final
/// multibyte scalar.
#[must_use]
pub fn str_truncated(s: &str, max_len: usize) -> tracing::field::DebugValue<TruncatedStr<'_>> {
	tracing::field::debug(TruncatedStr { inner: s, max_len })
}

/// Produces a debug label for an optional value without revealing its contents.
///
/// The macro expands to `"Some(<redacted>)"` when the identifier reports a
/// present value and to `"None"` otherwise. Its argument must be an identifier
/// supporting an `is_some` method.
#[macro_export]
macro_rules! redacted_debug {
	($f:ident) => {
		if $f.is_some() { "Some(<redacted>)" } else { "None" }
	};
}

impl<T: fmt::Debug> fmt::Debug for TruncatedSlice<'_, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.inner.len() <= self.max_len {
			write!(f, "{:?}", self.inner)
		} else {
			f.debug_list()
				.entries(&self.inner[..self.max_len])
				.entry(&"...")
				.finish()
		}
	}
}

impl fmt::Debug for TruncatedStr<'_> {
	#[expect(clippy::string_slice)]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if self.inner.len() <= self.max_len {
			write!(f, "{:?}", self.inner)
		} else {
			let len = self
				.inner
				.char_indices()
				.skip_while(|(i, _)| *i < self.max_len)
				.map(|(i, _)| i)
				.next()
				.expect("At least one char_indice >= len for str");

			write!(f, "{:?}...", &self.inner[..len])
		}
	}
}
