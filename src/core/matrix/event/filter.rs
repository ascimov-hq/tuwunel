use std::mem::take;

use ruma::{
	RoomId, UserId,
	api::client::filter::{Filter, RoomEventFilter, RoomFilter, UrlFilter},
	serde::Raw,
};
use serde_json::{Map, Value};
use smallvec::SmallVec;

use super::Event;
use crate::is_equal_to;

/// Segments of one `event_fields` entry; most paths are one or two deep.
type FieldPath = SmallVec<[String; 2]>;

pub trait Matches<T> {
	fn matches(&self, t: T) -> bool;
}

impl<E: Event> Matches<&E> for RoomEventFilter {
	#[inline]
	fn matches(&self, event: &E) -> bool {
		if !matches_sender(event, self) {
			return false;
		}

		if !matches_room(event, self) {
			return false;
		}

		if !matches_type(event, self) {
			return false;
		}

		if !matches_url(event, self) {
			return false;
		}

		true
	}
}

/// Event-type filtering by name, for data which is not a PDU: ephemeral
/// events, account data and presence.
pub trait MatchesType {
	fn matches_type(&self, kind: &str) -> bool;

	/// False when the filter excludes every event type (`not_types: ["*"]` or
	/// an empty `types`), so callers can skip loading that data at all.
	fn matches_any_type(&self) -> bool;
}

impl MatchesType for RoomEventFilter {
	#[inline]
	fn matches_type(&self, kind: &str) -> bool {
		matches_type_str(kind, self.types.as_deref(), &self.not_types)
	}

	#[inline]
	fn matches_any_type(&self) -> bool {
		matches_any_type(self.types.as_deref(), &self.not_types)
	}
}

impl MatchesType for Filter {
	#[inline]
	fn matches_type(&self, kind: &str) -> bool {
		matches_type_str(kind, self.types.as_deref(), &self.not_types)
	}

	#[inline]
	fn matches_any_type(&self) -> bool {
		matches_any_type(self.types.as_deref(), &self.not_types)
	}
}

impl Matches<&RoomId> for RoomFilter {
	#[inline]
	fn matches(&self, room_id: &RoomId) -> bool {
		if !matches_room_id(room_id, self) {
			return false;
		}

		true
	}
}

impl Matches<&UserId> for Filter {
	#[inline]
	fn matches(&self, user_id: &UserId) -> bool {
		if !matches_user_id(user_id, self) {
			return false;
		}

		true
	}
}

fn matches_user_id(user_id: &UserId, filter: &Filter) -> bool {
	if filter
		.not_senders
		.iter()
		.any(is_equal_to!(user_id))
	{
		return false;
	}

	if let Some(senders) = filter.senders.as_ref()
		&& !senders.iter().any(is_equal_to!(user_id))
	{
		return false;
	}

	true
}

fn matches_room_id(room_id: &RoomId, filter: &RoomFilter) -> bool {
	if filter.not_rooms.iter().any(is_equal_to!(room_id)) {
		return false;
	}

	if let Some(rooms) = filter.rooms.as_ref()
		&& !rooms.iter().any(is_equal_to!(room_id))
	{
		return false;
	}

	true
}

fn matches_room<E: Event>(event: &E, filter: &RoomEventFilter) -> bool {
	if filter
		.not_rooms
		.iter()
		.any(is_equal_to!(event.room_id()))
	{
		return false;
	}

	if let Some(rooms) = filter.rooms.as_ref()
		&& !rooms.iter().any(is_equal_to!(event.room_id()))
	{
		return false;
	}

	true
}

fn matches_sender<E: Event>(event: &E, filter: &RoomEventFilter) -> bool {
	if filter
		.not_senders
		.iter()
		.any(is_equal_to!(event.sender()))
	{
		return false;
	}

	if let Some(senders) = filter.senders.as_ref()
		&& !senders.iter().any(is_equal_to!(event.sender()))
	{
		return false;
	}

	true
}

fn matches_type<E: Event>(event: &E, filter: &RoomEventFilter) -> bool {
	let kind = event.kind().to_cow_str();

	matches_type_str(&kind, filter.types.as_deref(), &filter.not_types)
}

fn matches_type_str(kind: &str, types: Option<&[String]>, not_types: &[String]) -> bool {
	if not_types
		.iter()
		.any(|pattern| type_pattern_matches(pattern, kind))
	{
		return false;
	}

	if let Some(types) = types
		&& !types
			.iter()
			.any(|pattern| type_pattern_matches(pattern, kind))
	{
		return false;
	}

	true
}

fn matches_any_type(types: Option<&[String]>, not_types: &[String]) -> bool {
	!not_types.iter().any(|pattern| pattern == "*") && types.is_none_or(|types| !types.is_empty())
}

/// Match one `types`/`not_types` entry against an event type. Per the spec a
/// `*` is a wildcard matching any sequence of characters (so `*` alone matches
/// every type and `m.room.*` matches `m.room.member`); entries without a `*`
/// keep the exact comparison.
fn type_pattern_matches(pattern: &str, kind: &str) -> bool {
	if !pattern.contains('*') {
		return pattern == kind;
	}

	let mut segments = pattern.split('*');
	let first = segments.next().unwrap_or_default();
	let last = segments.next_back().unwrap_or_default();
	let Some(mut rest) = kind.strip_prefix(first) else {
		return false;
	};

	// Inner segments are matched leftmost; with `*` as the only metacharacter
	// the earliest occurrence leaves the most room for what follows.
	for segment in segments {
		let Some((_, after)) = rest.split_once(segment) else {
			return false;
		};

		rest = after;
	}

	rest.ends_with(last)
}

fn matches_url<E: Event>(event: &E, filter: &RoomEventFilter) -> bool {
	let Some(url_filter) = filter.url_filter.as_ref() else {
		return true;
	};

	//TODO: might be better to use Ruma's Raw rather than serde here
	let url = event
		.get_content_as_value()
		.get("url")
		.is_some_and(Value::is_string);

	match url_filter {
		| UrlFilter::EventsWithUrl => url,
		| UrlFilter::EventsWithoutUrl => !url,
	}
}

/// Restrict a serialized event to the requested `event_fields` per MSC3980,
/// returning it unchanged when no filter is set so the common sync pays
/// nothing.
///
/// The deserialize and reserialize happen only when `event_fields` is `Some`.
#[must_use]
pub fn trim_event_fields<T>(raw: Raw<T>, event_fields: Option<&[String]>) -> Raw<T> {
	// Identity keys clients rely on are always kept; the spec permits a superset.
	const ALWAYS_KEEP: [&str; 6] =
		["event_id", "type", "sender", "origin_server_ts", "state_key", "room_id"];

	let Some(fields) = event_fields else {
		return raw;
	};

	let Ok(Value::Object(source)) = raw.deserialize_as::<Value>() else {
		return raw;
	};

	let mut out: Map<String, Value> = ALWAYS_KEEP
		.into_iter()
		.filter_map(|key| {
			source
				.get(key)
				.map(|value| (key.to_owned(), value.clone()))
		})
		.collect();

	for field in fields {
		copy_field_path(&source, &mut out, &split_field(field));
	}

	Raw::from_json_value(&Value::Object(out))
}

/// Copy the leaf or subtree at `segments` from `source` into the same nested
/// path in `out`, creating intermediate objects as needed; an absent or
/// non-object path is skipped.
fn copy_field_path(
	source: &Map<String, Value>,
	out: &mut Map<String, Value>,
	segments: &[String],
) {
	let Some((head, rest)) = segments.split_first() else {
		return;
	};

	let Some(value) = source.get(head) else {
		return;
	};

	if rest.is_empty() {
		out.insert(head.clone(), value.clone());
		return;
	}

	let Value::Object(child) = value else {
		return;
	};

	if let Value::Object(child_out) = out
		.entry(head.clone())
		.or_insert_with(|| Value::Object(Map::new()))
	{
		copy_field_path(child, child_out, rest);
	}
}

/// Split one `event_fields` entry into its segments, inverting the MSC3873
/// escape grammar used by ruma's `escape_key`: `\.` is a literal dot inside the
/// current segment, `\\` a literal backslash, any other `\X` is kept verbatim,
/// and an unescaped `.` ends the segment.
fn split_field(field: &str) -> FieldPath {
	let mut segments = FieldPath::new();
	let mut current = String::new();
	let mut chars = field.chars();
	while let Some(c) = chars.next() {
		match c {
			| '\\' => match chars.next() {
				| Some('.') => current.push('.'),
				| Some('\\') | None => current.push('\\'),
				| Some(other) => {
					current.push('\\');
					current.push(other);
				},
			},
			| '.' => {
				segments.push(take(&mut current));
			},
			| other => current.push(other),
		}
	}

	segments.push(current);
	segments
}

#[cfg(test)]
mod tests {
	use super::{split_field, type_pattern_matches};

	#[test]
	fn type_pattern_exact() {
		assert!(type_pattern_matches("m.room.message", "m.room.message"));
		assert!(!type_pattern_matches("m.room.message", "m.room.messages"));
	}

	#[test]
	fn type_pattern_wildcards() {
		assert!(type_pattern_matches("*", "m.room.member"));
		assert!(type_pattern_matches("*", ""));
		assert!(type_pattern_matches("m.room.*", "m.room.member"));
		assert!(!type_pattern_matches("m.room.*", "m.reaction"));
		assert!(type_pattern_matches("*.member", "m.room.member"));
		assert!(type_pattern_matches("m.*.m*r", "m.room.member"));
		assert!(!type_pattern_matches("m.*.x*r", "m.room.member"));
		assert!(type_pattern_matches("a*a", "aa"));
		assert!(!type_pattern_matches("a*a", "a"));
	}

	fn segments(field: &str) -> Vec<String> { split_field(field).into_vec() }

	#[test]
	fn split_simple_path() {
		assert_eq!(segments("content.body"), ["content", "body"]);
	}

	#[test]
	fn split_escaped_dot() {
		assert_eq!(segments(r"content.m\.relates_to"), ["content", "m.relates_to"]);
	}

	#[test]
	fn split_escaped_backslash_then_dot() {
		assert_eq!(segments(r"a\\.b"), [r"a\", "b"]);
	}

	#[test]
	fn split_reserved_escape_kept_verbatim() {
		assert_eq!(segments(r"weird\x"), [r"weird\x"]);
	}

	#[test]
	fn split_trailing_backslash() {
		assert_eq!(segments(r"foo\"), [r"foo\"]);
	}
}
