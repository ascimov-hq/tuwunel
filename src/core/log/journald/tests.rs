#![expect(
	clippy::little_endian_bytes,
	reason = "the journal protocol specifies little-endian field lengths"
)]

use super::{Buffer, LEN_PREFIX, boundary, close, put, sanitize};

#[test]
fn put_field_framing() {
	let mut payload = Buffer::new();

	put(&mut payload, "TARGET", b"tuwunel_service::sending");

	let (name, rest) = payload.split_at(7);
	let (len, value) = rest.split_at(LEN_PREFIX);

	assert_eq!(name, b"TARGET\n");
	assert_eq!(u64::from_le_bytes(len.try_into().unwrap()), 24);
	assert_eq!(value, b"tuwunel_service::sending\n");
}

#[test]
fn put_field_accepts_newlines() {
	let mut payload = Buffer::new();

	put(&mut payload, "MESSAGE", b"two\nlines");

	let len = &payload[8..8 + LEN_PREFIX];

	assert_eq!(u64::from_le_bytes(len.try_into().unwrap()), 9);
	assert_eq!(payload.last(), Some(&b'\n'));
}

#[test]
fn close_seals_message_length() {
	let mut payload = Buffer::new();

	put(&mut payload, "PRIORITY", b"4");
	payload.extend_from_slice(b"MESSAGE\n");
	payload.extend_from_slice(&[0; LEN_PREFIX]);

	let message = payload.len();

	payload.extend_from_slice(b"a warning\n");

	close(&mut payload, message);

	let len = &payload[message - LEN_PREFIX..message];

	assert_eq!(u64::from_le_bytes(len.try_into().unwrap()), 9);
	assert_eq!(&payload[message..], b"a warning\n");
}

#[test]
fn close_trims_trailing_whitespace() {
	let mut payload = Buffer::new();

	payload.extend_from_slice(&[0; LEN_PREFIX]);

	let message = payload.len();

	payload.extend_from_slice(b"  indented  \n");

	close(&mut payload, message);

	assert_eq!(&payload[message..], b"  indented\n");
}

#[test]
fn sanitize_field_names() {
	assert_eq!(sanitize("room_id"), "F_ROOM_ID");
	assert_eq!(sanitize("time.busy"), "F_TIME_BUSY");
	assert_eq!(sanitize("otel.status-code"), "F_OTEL_STATUS_CODE");
}

#[test]
fn boundary_steps_off_continuation_bytes() {
	let message = "ab\u{1F600}".as_bytes();

	assert_eq!(boundary(message, 6), 6);
	assert_eq!(boundary(message, 5), 2);
	assert_eq!(boundary(message, 2), 2);
	assert_eq!(boundary(message, 0), 0);
}
