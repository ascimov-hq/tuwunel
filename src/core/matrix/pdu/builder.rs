use std::{collections::BTreeMap, fmt};

use ruma::{
	MilliSecondsSinceUnixEpoch, OwnedEventId,
	events::{MessageLikeEventContent, StateEventContent, TimelineEventType},
};
use serde::Deserialize;
use serde_json::value::{RawValue as RawJsonValue, to_raw_value};

use super::{Content, StateKey};

tokio::task_local! {
	/// Set while appending events which must not notify anyone, e.g. history
	/// sent through `com.beeper.batch_sending` with `send_notification: false`.
	/// The pusher then skips push-rule evaluation, notification counts and
	/// pushes for those events. Unset means notify as usual.
	pub static SUPPRESS_NOTIFICATIONS: bool;
}

/// Whether the current task is appending events under
/// [`SUPPRESS_NOTIFICATIONS`].
#[must_use]
pub fn notifications_suppressed() -> bool {
	SUPPRESS_NOTIFICATIONS
		.try_with(|suppressed| *suppressed)
		.unwrap_or(false)
}

/// Build the start of a PDU in order to add it to the Database.
#[derive(Deserialize)]
pub struct Builder {
	#[serde(rename = "type")]
	pub event_type: TimelineEventType,

	pub content: Content,

	pub unsigned: Option<BTreeMap<String, serde_json::Value>>,

	pub state_key: Option<StateKey>,

	pub redacts: Option<OwnedEventId>,

	/// For timestamped messaging, should only be used for appservices.
	/// Will be set to current time if None
	pub timestamp: Option<MilliSecondsSinceUnixEpoch>,
}

impl Default for Builder {
	fn default() -> Self {
		Self {
			event_type: "m.room.message".into(),
			content: Box::<RawJsonValue>::default().into(),
			unsigned: None,
			state_key: None,
			redacts: None,
			timestamp: None,
		}
	}
}

impl Builder {
	pub fn state<S, T>(state_key: S, content: &T) -> Self
	where
		T: StateEventContent,
		S: Into<StateKey>,
	{
		Self {
			event_type: content.event_type().into(),
			content: to_raw_value(content)
				.map(Into::into)
				.expect("Builder failed to serialize state event content to RawValue"),
			state_key: Some(state_key.into()),
			..Self::default()
		}
	}

	pub fn timeline<T>(content: &T) -> Self
	where
		T: MessageLikeEventContent,
	{
		Self {
			event_type: content.event_type().into(),
			content: to_raw_value(content)
				.map(Into::into)
				.expect("Builder failed to serialize timeline event content to RawValue"),
			..Self::default()
		}
	}
}

impl fmt::Debug for Builder {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut d = f.debug_struct("Builder");

		d.field("type", &self.event_type);

		if let Some(state_key) = self.state_key.as_ref() {
			d.field("state_key", state_key);
		}

		if let Some(redacts) = self.redacts.as_ref() {
			d.field("redacts", redacts);
		}

		if let Some(timestamp) = self.timestamp.as_ref() {
			d.field("ts", timestamp);
		}

		if let Some(unsigned) = self.unsigned.as_ref() {
			d.field("unsigned", unsigned);
		}

		d.field("content", &self.content);

		d.finish()
	}
}
