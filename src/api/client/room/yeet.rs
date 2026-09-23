use axum::extract::State;
use ruma::api::client::tuwunel::delete_room_yeet::unstable as delete_room_yeet;
use tuwunel_core::Result;

use crate::Ruma;

/// # `POST /_matrix/client/unstable/com.beeper.yeet/rooms/{roomId}/delete`
///
/// Beeper's room-yeeting extension: fully purges a room from the homeserver,
/// rather than just emptying it. Callable only by appservices (enforced by
/// the endpoint's `AppserviceToken` authentication scheme). Idempotent: an
/// unknown or already-deleted room still returns 200.
pub(crate) async fn delete_room_yeet_route(
	State(services): State<crate::State>,
	body: Ruma<delete_room_yeet::Request>,
) -> Result<delete_room_yeet::Response> {
	let state_lock = services.state.mutex.lock(&body.room_id).await;

	services
		.delete
		.delete_room(&body.room_id, false, state_lock)
		.await
		.ok();

	Ok(delete_room_yeet::Response::new())
}
