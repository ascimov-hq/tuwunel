use std::{borrow::Cow, fmt::Debug, mem};

use bytes::{Bytes, BytesMut};
use reqwest::{Request, StatusCode};
use ruma::api::{
	IncomingResponse, OutgoingRequest, OutgoingRequestExt,
	appservice::Registration,
	auth_scheme::{AuthScheme, SendAccessToken},
	path_builder::PathBuilder,
};
use tuwunel_core::{
	Err, Result, debug, debug_error, err, implement, trace, utils::string_from_bytes, warn,
};

use crate::client::read_response_capped;

/// Sends a request to an appservice
///
/// Only returns Ok(None) if there is no url specified in the appservice
/// registration file
#[implement(super::Service)]
pub async fn send_request<T>(
	&self,
	registration: Registration,
	request: T,
) -> Result<Option<T::IncomingResponse>>
where
	T: OutgoingRequest + Debug + Send,
	for<'a> T::Authentication: AuthScheme<Input<'a> = SendAccessToken<'a>>,
	for<'a> T::PathBuilder: PathBuilder<Input<'a> = ()>,
{
	self.send_request_to_route(registration, request, false)
		.await
}

/// Sends a request to an optional, unstable appservice route (the MSC3983 and
/// MSC3984 key proxies). An appservice that does not serve the route answers
/// 404, 405 or `M_UNRECOGNIZED`. That answer is expected, so it is logged at
/// debug level and returns Ok(None) instead of a warning and an error.
#[implement(super::Service)]
pub(super) async fn send_optional_request<T>(
	&self,
	registration: Registration,
	request: T,
) -> Result<Option<T::IncomingResponse>>
where
	T: OutgoingRequest + Debug + Send,
	for<'a> T::Authentication: AuthScheme<Input<'a> = SendAccessToken<'a>>,
	for<'a> T::PathBuilder: PathBuilder<Input<'a> = ()>,
{
	self.send_request_to_route(registration, request, true)
		.await
}

#[implement(super::Service)]
async fn send_request_to_route<T>(
	&self,
	registration: Registration,
	request: T,
	optional_route: bool,
) -> Result<Option<T::IncomingResponse>>
where
	T: OutgoingRequest + Debug + Send,
	for<'a> T::Authentication: AuthScheme<Input<'a> = SendAccessToken<'a>>,
	for<'a> T::PathBuilder: PathBuilder<Input<'a> = ()>,
{
	let client = &self.services.client.appservice;

	let Some(dest) = registration.url else {
		return Ok(None);
	};

	if dest == *"null" || dest.is_empty() {
		return Ok(None);
	}

	trace!("Appservice URL \"{dest}\", Appservice ID: {}", registration.id);

	let hs_token = registration.hs_token.as_str();
	let mut http_request = request
		.try_into_http_request::<BytesMut>(&dest, SendAccessToken::IfRequired(hs_token), ())
		.map_err(|e| {
			err!(BadServerResponse(
				warn!(appservice = %registration.id, "Failed to find destination {dest}: {e:?}")
			))
		})?
		.map(BytesMut::freeze);

	add_access_token_query(&mut http_request, hs_token);

	let reqwest_request = Request::try_from(http_request)?;

	let mut response = client
		.execute(reqwest_request)
		.await
		.map_err(|e| {
			// The error carries the request URL, which includes the hs_token as
			// `?access_token=`; redact it before it is logged or propagated.
			let e = redact_error_url(e);
			warn!(
				"Could not send request to appservice \"{}\" at {dest}: {e:?}",
				registration.id
			);
			e
		})?;

	// reqwest::Response -> http::Response conversion
	let status = response.status();
	let mut http_response_builder = http::Response::builder()
		.status(status)
		.version(response.version());

	mem::swap(
		response.headers_mut(),
		http_response_builder
			.headers_mut()
			.expect("http::response::Builder is usable"),
	);

	let limit = self.services.config.max_response_size;
	let body = read_response_capped(response, limit)
		.await
		.map_err(|e| {
			let e = redact_access_token(&e.to_string()).into_owned();
			err!(BadServerResponse(warn!(
				"Failed reading response from appservice \"{}\" at {dest}: {e}",
				registration.id
			)))
		})?;

	if !status.is_success() {
		if optional_route && is_unrecognized_route(status, &body) {
			debug!(
				appservice = %registration.id,
				%status,
				"Appservice does not serve the optional route of {}",
				std::any::type_name::<T>(),
			);

			return Ok(None);
		}

		debug_error!("Appservice response bytes: {:?}", string_from_bytes(&body));
		return Err!(BadServerResponse(warn!(
			"Appservice \"{}\" returned unsuccessful HTTP response {status} at {dest}",
			registration.id
		)));
	}

	let response = T::IncomingResponse::try_from_http_response(
		http_response_builder
			.body(body)
			.expect("reqwest body is valid http body"),
	);

	response.map(Some).map_err(|e| {
		err!(BadServerResponse(warn!(
			"Appservice \"{}\" returned invalid/malformed response bytes {dest}: {e}",
			registration.id
		)))
	})
}

/// Whether an unsuccessful response says the appservice does not serve the
/// route: 404 or 405 with any body, or the `M_UNRECOGNIZED` error code with
/// any 4xx status (Synapse's `is_unknown_endpoint` accepts 400 for older
/// servers).
fn is_unrecognized_route(status: StatusCode, body: &[u8]) -> bool {
	matches!(status, StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED)
		|| (status.is_client_error()
			&& serde_json::from_slice::<serde_json::Value>(body)
				.ok()
				.as_ref()
				.and_then(|body| body.get("errcode"))
				.and_then(serde_json::Value::as_str)
				.is_some_and(|errcode| errcode == "M_UNRECOGNIZED"))
}

/// Appends the `hs_token` as an `access_token` query parameter, the legacy
/// authentication scheme some appservices still require.
pub(super) fn add_access_token_query(request: &mut http::Request<Bytes>, hs_token: &str) {
	let mut parts = request.uri().clone().into_parts();
	let old_path_and_query = parts
		.path_and_query
		.expect("valid request uri path and query")
		.as_str()
		.to_owned();

	let symbol = if old_path_and_query.contains('?') { "&" } else { "?" };

	parts.path_and_query = Some(
		format!("{old_path_and_query}{symbol}access_token={hs_token}")
			.parse()
			.expect("valid path and query"),
	);

	*request.uri_mut() = parts
		.try_into()
		.expect("our manipulation is always valid");
}

/// Placeholder replacing the `access_token` value in anything we log.
const REDACTED: &str = "<redacted>";

/// Redacts the `access_token` query parameter (the hs_token appended by
/// [`add_access_token_query`]) from the URL a reqwest error carries, so that
/// neither its `Debug`/`Display` output nor the error propagated to callers
/// (and logged again by the sender) reveals the token.
pub(super) fn redact_error_url(mut error: reqwest::Error) -> reqwest::Error {
	if let Some(url) = error.url_mut() {
		redact_url(url);
	}

	error
}

fn redact_url(url: &mut reqwest::Url) {
	if !url
		.query_pairs()
		.any(|(name, _)| name == "access_token")
	{
		return;
	}

	let pairs: Vec<(String, String)> = url
		.query_pairs()
		.map(|(name, value)| {
			let value = if name == "access_token" {
				REDACTED.into()
			} else {
				value.into_owned()
			};

			(name.into_owned(), value)
		})
		.collect();

	url.query_pairs_mut().clear().extend_pairs(pairs);
}

/// Redacts every `access_token=` value in a message that may embed a request
/// URL (e.g. the `Display` of a transport error).
pub(super) fn redact_access_token(message: &str) -> Cow<'_, str> {
	const NEEDLE: &str = "access_token=";

	if !message.contains(NEEDLE) {
		return Cow::Borrowed(message);
	}

	let mut out = String::with_capacity(message.len());
	let mut rest = message;
	while let Some(pos) = rest.find(NEEDLE) {
		let (head, tail) = rest.split_at(pos.saturating_add(NEEDLE.len()));
		out.push_str(head);
		out.push_str(REDACTED);

		let end = tail
			.find(|c: char| {
				c.is_whitespace() || matches!(c, '&' | '#' | '"' | '\'' | ')' | '>' | ',')
			})
			.unwrap_or(tail.len());

		rest = tail.get(end..).unwrap_or_default();
	}

	out.push_str(rest);
	Cow::Owned(out)
}

#[cfg(test)]
mod tests {
	use reqwest::StatusCode;

	use super::{is_unrecognized_route, redact_access_token, redact_url};

	#[test]
	fn unrecognized_route_answers() {
		// mautrix-go (gorilla/mux) answers an unknown route with a plain 404.
		assert!(is_unrecognized_route(StatusCode::NOT_FOUND, b"404 page not found\n"));
		assert!(is_unrecognized_route(
			StatusCode::NOT_FOUND,
			br#"{"errcode":"M_UNRECOGNIZED","error":"Unrecognized request"}"#
		));
		assert!(is_unrecognized_route(StatusCode::METHOD_NOT_ALLOWED, b""));
		assert!(is_unrecognized_route(
			StatusCode::BAD_REQUEST,
			br#"{"errcode":"M_UNRECOGNIZED"}"#
		));
	}

	#[test]
	fn other_errors_are_not_unrecognized_routes() {
		assert!(!is_unrecognized_route(StatusCode::BAD_REQUEST, br#"{"errcode":"M_BAD_JSON"}"#));
		assert!(!is_unrecognized_route(StatusCode::FORBIDDEN, br#"{"errcode":"M_FORBIDDEN"}"#));
		assert!(!is_unrecognized_route(StatusCode::BAD_REQUEST, b"not json"));
		assert!(!is_unrecognized_route(
			StatusCode::INTERNAL_SERVER_ERROR,
			br#"{"errcode":"M_UNRECOGNIZED"}"#
		));
		assert!(!is_unrecognized_route(StatusCode::BAD_GATEWAY, b""));
	}

	#[test]
	fn redacts_url_query() {
		let mut url: reqwest::Url =
			"http://whatsapp:29318/_matrix/app/v1/transactions/42?access_token=secret&x=1"
				.parse()
				.unwrap();

		redact_url(&mut url);
		assert!(!url.as_str().contains("secret"));
		assert!(url.as_str().contains("x=1"));
		assert!(url.as_str().contains("/transactions/42"));
	}

	#[test]
	fn redacts_message() {
		let msg = "error sending request for url (http://h/_matrix/app/v1/transactions/1?access_token=s3cr3t)";
		let out = redact_access_token(msg);
		assert!(!out.contains("s3cr3t"));
		assert!(out.ends_with("access_token=<redacted>)"));
		assert_eq!(redact_access_token("no token here"), "no token here");
	}
}
