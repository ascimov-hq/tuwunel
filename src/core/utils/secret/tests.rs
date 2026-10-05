use std::{
	env::temp_dir,
	fs::{remove_file, write},
	path::PathBuf,
};

use super::resolve;

const NAME: &str = "test secret";

fn secret_file(name: &str, contents: &str) -> PathBuf {
	let path = temp_dir().join(name);
	write(&path, contents).expect("temp secret file written");

	path
}

#[test]
fn inline_resolves() {
	assert_eq!(resolve(None, Some("inline"), NAME).as_deref(), Some("inline"));
}

#[test]
fn nothing_configured_resolves_to_nothing() {
	assert_eq!(resolve(None, None, NAME), None);
}

#[test]
fn empty_inline_resolves_to_nothing() {
	assert_eq!(resolve(None, Some(""), NAME), None);
}

#[test]
fn secret_longer_than_the_inline_budget_survives() {
	let long = "x".repeat(200);

	assert_eq!(resolve(None, Some(&long), NAME).as_deref(), Some(long.as_str()));
}

#[test]
fn inline_is_not_trimmed() {
	assert_eq!(resolve(None, Some(" spaced "), NAME).as_deref(), Some(" spaced "));
}

#[test]
fn file_takes_precedence_over_inline() {
	let path = secret_file("tuwunel_secret_precedence", "from-file");

	assert_eq!(resolve(Some(&path), Some("inline"), NAME).as_deref(), Some("from-file"));

	remove_file(&path).expect("temp secret file removed");
}

#[test]
fn file_contents_are_trimmed() {
	let path = secret_file("tuwunel_secret_trimmed", "  from-file\n");

	assert_eq!(resolve(Some(&path), None, NAME).as_deref(), Some("from-file"));

	remove_file(&path).expect("temp secret file removed");
}

#[test]
fn blank_file_does_not_fall_back() {
	let path = secret_file("tuwunel_secret_blank", "\n\n");

	assert_eq!(resolve(Some(&path), Some("inline"), NAME), None);

	remove_file(&path).expect("temp secret file removed");
}

#[test]
fn unreadable_file_falls_back() {
	let path = temp_dir().join("tuwunel_secret_absent");

	assert_eq!(resolve(Some(&path), Some("inline"), NAME).as_deref(), Some("inline"));
}
