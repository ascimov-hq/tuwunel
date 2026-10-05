use std::collections::BTreeMap;

use futures::StreamExt;
use ruma::{
	OneTimeKeyAlgorithm, OwnedDeviceId, OwnedOneTimeKeyId, UserId,
	api::appservice::{
		Registration,
		keys::{
			claim_keys::unstable::Request as ClaimRequest,
			query_keys::unstable::Request as QueryRequest,
		},
	},
	encryption::{DeviceKeys, OneTimeKey},
	serde::Raw,
};
use tuwunel_core::{
	implement,
	smallvec::SmallVec,
	utils::{IterStream, stream::ReadyExt},
};

type ClaimedKeys = BTreeMap<OwnedDeviceId, BTreeMap<OwnedOneTimeKeyId, Raw<OneTimeKey>>>;
type QueriedKeys = BTreeMap<OwnedDeviceId, Raw<DeviceKeys>>;
type Registrations = SmallVec<[Registration; 1]>;

/// MSC3983: claims one-time keys for `user_id` from the appservices that opted
/// in through `appservice_key_proxy`. When none did (the default), no
/// appservice is asked and the result is empty.
#[implement(super::Service)]
#[tracing::instrument(level = "debug", skip(self, one_time_keys))]
pub async fn claim_keys(
	&self,
	user_id: &UserId,
	one_time_keys: &BTreeMap<OwnedDeviceId, OneTimeKeyAlgorithm>,
) -> ClaimedKeys {
	self.key_proxy_registrations(user_id)
		.await
		.into_iter()
		.stream()
		.filter_map(async |registration| {
			let devices = one_time_keys
				.iter()
				.map(|(device_id, algorithm)| (device_id.clone(), vec![algorithm.clone()]))
				.collect();

			let request = ClaimRequest {
				one_time_keys: [(user_id.to_owned(), devices)].into(),
			};

			self.send_optional_request(registration, request)
				.await
				.ok()
				.flatten()
				.and_then(|mut response| response.one_time_keys.remove(user_id))
		})
		.ready_fold(ClaimedKeys::new(), |claimed, response| {
			response
				.into_iter()
				.fold(claimed, |mut claimed, (device_id, keys)| {
					let Some(algorithm) = one_time_keys.get(&device_id) else {
						return claimed;
					};

					let keys = keys
						.into_iter()
						.filter(|(key_id, _)| key_id.algorithm() == *algorithm)
						.take(1)
						.collect::<BTreeMap<_, _>>();

					if !keys.is_empty() {
						claimed.insert(device_id, keys);
					}

					claimed
				})
		})
		.await
}

/// MSC3984: queries device keys of `user_id` from the appservices that opted
/// in through `appservice_key_proxy`. When none did (the default), no
/// appservice is asked and the result is empty.
#[implement(super::Service)]
#[tracing::instrument(level = "debug", skip(self, devices))]
pub async fn query_keys(&self, user_id: &UserId, devices: &[OwnedDeviceId]) -> QueriedKeys {
	self.key_proxy_registrations(user_id)
		.await
		.into_iter()
		.stream()
		.filter_map(async |registration| {
			let request = QueryRequest {
				device_keys: [(user_id.to_owned(), devices.to_vec())].into(),
			};

			self.send_optional_request(registration, request)
				.await
				.ok()
				.flatten()
				.and_then(|mut response| response.device_keys.remove(user_id))
		})
		.ready_fold(QueriedKeys::new(), |mut queried, response| {
			queried.extend(response);
			queried
		})
		.await
}

/// The registrations a key claim or key query for `user_id` goes to: those
/// listed in `appservice_key_proxy` that claim `user_id` exclusively (or whose
/// sender it is). With the default empty list this returns at once, without
/// taking the registrations lock or cloning any registration.
#[implement(super::Service)]
async fn key_proxy_registrations(&self, user_id: &UserId) -> Registrations {
	if self
		.services
		.config
		.appservice_key_proxy
		.is_empty()
	{
		return Registrations::new();
	}

	let registrations = self.read().await;

	// Read the config only after the await: references into it must not be
	// held across one (see tuwunel_core::config::Manager).
	let opted_in = &self.services.config.appservice_key_proxy;

	registrations
		.values()
		.filter(|info| info.is_key_proxy_match(opted_in, user_id))
		.map(|info| info.registration.clone())
		.collect()
}
