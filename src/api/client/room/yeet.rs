use axum::extract::State;
use ruma::RoomId;
use tuwunel_core::{Err, Result, debug_warn, err, matrix::room_version, utils::result::LogErr};
use tuwunel_service::appservice::RegistrationInfo;

use self::endpoint::unstable as delete_room_yeet;
use crate::Ruma;

/// # `POST /_matrix/client/unstable/com.beeper.yeet/rooms/{roomId}/delete`
///
/// Beeper's room-yeeting extension: fully purges a room from the homeserver,
/// rather than just emptying it. Callable only by appservices (enforced by
/// the endpoint's `AppserviceToken` authentication scheme), and only for rooms
/// created by the calling appservice: its sender (bot) or a user in its
/// exclusive user namespace must be a creator of the room, otherwise
/// `M_FORBIDDEN`. Idempotent: an unknown or already-deleted room still
/// returns 200.
pub(crate) async fn delete_room_yeet_route(
	State(services): State<crate::State>,
	body: Ruma<delete_room_yeet::Request>,
) -> Result<delete_room_yeet::Response> {
	let appservice = body
		.appservice_info
		.as_ref()
		.ok_or_else(|| err!(Request(Forbidden("Only appservices may yeet rooms."))))?;

	let state_lock = services.state.mutex.lock(&body.room_id).await;

	if !services.metadata.exists(&body.room_id).await {
		return Ok(delete_room_yeet::Response::new());
	}

	if !is_room_owned_by(&services, &body.room_id, appservice).await? {
		debug_warn!(
			room_id = %body.room_id,
			appservice = %appservice.registration.id,
			"Refusing to yeet a room not created by the calling appservice"
		);

		return Err!(Request(Forbidden(
			"The room was not created by this appservice or one of its users."
		)));
	}

	// Idempotent for the caller (200 either way, as before), but a failed purge
	// must not go unnoticed by the operator.
	services
		.delete
		.delete_room(&body.room_id, false, state_lock)
		.await
		.log_err()
		.ok();

	Ok(delete_room_yeet::Response::new())
}

/// Whether one of the room's creators is the appservice's sender or a user in
/// its exclusive user namespace. Non-exclusive namespaces (e.g. a
/// double-puppeting registration matching every local user) do not grant
/// ownership.
async fn is_room_owned_by(
	services: &crate::State,
	room_id: &RoomId,
	appservice: &RegistrationInfo,
) -> Result<bool> {
	let Ok(create) = services.state_accessor.get_create(room_id).await else {
		// No create event in our state: nothing we could attribute to the caller.
		return Ok(false);
	};

	let room_version = create.room_version()?;
	let rules = room_version::rules(&room_version)?;
	let owned = create
		.creators(&rules.authorization)?
		.any(|creator| appservice.is_exclusive_user_match(&creator));

	Ok(owned)
}

/// Endpoint definition for Beeper's room-yeeting extension. Defined locally
/// so Tuwunel builds against upstream ruma without a fork; the path, method
/// and authentication are identical to the former `ascimov-hq/ruma`
/// definition (`ruma_client_api::tuwunel::delete_room_yeet`).
pub(crate) mod endpoint {
	pub(crate) mod unstable {
		//! `/unstable/` (Beeper's `com.beeper.yeet` room-yeeting extension, no
		//! published MSC)

		use ruma::{
			OwnedRoomId,
			api::{auth_scheme::AppserviceToken, metadata, request, response},
		};

		metadata! {
			method: POST,
			rate_limited: false,
			authentication: AppserviceToken,
			history: {
				unstable => "/_matrix/client/unstable/com.beeper.yeet/rooms/{room_id}/delete",
			}
		}

		/// Request type for the `delete_room_yeet` endpoint.
		#[request]
		pub(crate) struct Request {
			/// The room to purge.
			#[ruma_api(path)]
			pub(crate) room_id: OwnedRoomId,
		}

		/// Response type for the `delete_room_yeet` endpoint.
		#[response]
		#[expect(clippy::empty_structs_with_brackets, reason = "ruma's #[response] form")]
		pub(crate) struct Response {}

		impl Response {
			/// Creates a new `Response`.
			#[must_use]
			pub(crate) fn new() -> Self { Self {} }
		}
	}
}
