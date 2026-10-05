//! Random value generation and randomized truncation helpers.
//!
//! These helpers use the thread-local generator for strings, indexes, shuffles,
//! durations, and event identifiers. Range arguments use half-open semantics.

use std::{
	iter::repeat_with,
	ops::Range,
	time::{Duration, SystemTime},
};

use arrayvec::ArrayString;
use rand::{RngExt, rng, seq::SliceRandom};
use ruma::OwnedEventId;

/// Randomly permutes a slice in place.
///
/// The thread-local generator chooses the permutation without changing the
/// slice's contents or length. Empty and single-element slices remain
/// unchanged.
pub fn shuffle<T>(vec: &mut [T]) {
	let mut rng = rng();
	vec.shuffle(&mut rng);
}

/// Chooses a uniformly random index below `len`.
///
/// A zero length returns `0` instead of sampling an empty range. For any
/// nonzero length, the result lies in `0..len`.
#[must_use]
pub fn index(len: usize) -> usize {
	match len {
		| 0 => 0,
		| len => rng().random_range(0..len),
	}
}

/// Generates an alphanumeric ASCII string of the requested byte length.
///
/// Each character is sampled independently with the thread-local generator.
/// Because the alphabet is ASCII, the character and byte lengths are equal.
pub fn string(length: usize) -> String {
	rng()
		.sample_iter(&rand::distr::Alphanumeric)
		.take(length)
		.map(char::from)
		.collect()
}

/// Generates a string of `length` characters sampled from `charset`.
///
/// Each byte becomes the Unicode scalar with the same numeric value, and
/// samples are uniform with replacement. Sampling panics when `charset` is
/// empty and a positive length is requested.
#[must_use]
pub fn string_from(charset: &[u8], length: usize) -> String {
	let mut rng = rng();

	repeat_with(|| char::from(charset[rng.random_range(0..charset.len())]))
		.take(length)
		.collect()
}

/// Generates an alphanumeric ASCII string that fills a fixed-capacity array.
///
/// Each sample occupies one byte, so the returned length and capacity are both
/// `LENGTH`. The [`ArrayString`] stores the result without heap allocation.
#[inline]
pub fn string_array<const LENGTH: usize>() -> ArrayString<LENGTH> {
	let mut ret = ArrayString::<LENGTH>::new();
	rng()
		.sample_iter(&rand::distr::Alphanumeric)
		.take(LENGTH)
		.map(char::from)
		.for_each(|c| ret.push(c));

	ret
}

/// Generates a Matrix event identifier from 32 random bytes.
///
/// The bytes use URL-safe base64 without padding, producing a 43-character
/// localpart after the `$` sigil. The identifier has no server-name component.
#[must_use]
pub fn event_id() -> OwnedEventId {
	use base64::{
		Engine,
		alphabet::URL_SAFE,
		engine::{GeneralPurpose, general_purpose::NO_PAD},
	};

	let mut binary: [u8; 32] = [0; _];
	rand::fill(&mut binary);

	let mut encoded: [u8; 43] = [0; _];
	GeneralPurpose::new(&URL_SAFE, NO_PAD)
		.encode_slice(binary, &mut encoded)
		.expect("Failed to encode binary to base64");

	let event_id: &str = str::from_utf8(&encoded)
		.expect("Failed to convert array of base64 bytes to valid utf8 str");

	OwnedEventId::from_parts('$', event_id, None)
		.expect("Failed to generate valid random event_id")
}

/// Truncates an owned string at a randomly selected character count.
///
/// The count is sampled from the half-open range and never splits a UTF-8
/// scalar. A count at or beyond the string's character count leaves it intact;
/// an invalid or empty range panics.
#[must_use]
pub fn truncate_string(mut str: String, range: Range<u64>) -> String {
	let len = rng()
		.random_range(range)
		.try_into()
		.unwrap_or(usize::MAX);

	if let Some((i, _)) = str.char_indices().nth(len) {
		str.truncate(i);
	}

	str
}

/// Borrows a prefix ending at a randomly selected character count.
///
/// The count is sampled from the half-open range and never splits a UTF-8
/// scalar. A count at or beyond the string's character count returns the full
/// input; an invalid or empty range panics.
#[inline]
#[must_use]
pub fn truncate_str(str: &str, range: Range<u64>) -> &str {
	let len = rng()
		.random_range(range)
		.try_into()
		.unwrap_or(usize::MAX);

	str.char_indices()
		.nth(len)
		.map(|(i, _)| str.split_at(i).0)
		.unwrap_or(str)
}

/// Adds a random whole-second offset to the current [`SystemTime`].
///
/// The offset is sampled from the supplied half-open range. The function panics
/// if the range is invalid or the addition exceeds [`SystemTime`].
#[inline]
#[must_use]
pub fn time_from_now_secs(range: Range<u64>) -> SystemTime {
	SystemTime::now()
		.checked_add(secs(range))
		.expect("range does not overflow SystemTime")
}

/// Generates a [`Duration`] with a random whole-second length.
///
/// The number of seconds is sampled uniformly from the supplied half-open
/// range. An invalid or empty range panics.
#[must_use]
pub fn secs(range: Range<u64>) -> Duration {
	let mut rng = rng();
	Duration::from_secs(rng.random_range(range))
}
