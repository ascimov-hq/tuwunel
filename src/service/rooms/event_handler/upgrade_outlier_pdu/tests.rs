use std::{ops::Range, time::Duration};

use tuwunel_core::utils::continue_exponential_backoff;

use super::UPGRADE_RETRY;

#[test]
fn upgrade_retry_releases_after_the_window() {
	let Range { start, end } = UPGRADE_RETRY;

	assert!(continue_exponential_backoff(start, end, Duration::from_mins(4), 1));
	assert!(!continue_exponential_backoff(start, end, Duration::from_mins(6), 1));
}

#[test]
fn upgrade_retry_widens_but_stays_capped() {
	let Range { start, end } = UPGRADE_RETRY;

	assert!(continue_exponential_backoff(start, end, Duration::from_mins(6), 2));
	assert!(!continue_exponential_backoff(start, end, end, 1_000));
}
