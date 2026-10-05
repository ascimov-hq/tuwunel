#![cfg(test)]

use std::{fs::remove_dir_all, process::id as process_id};

use tuwunel::{Args, Runtime, Server};
use tuwunel_core::{Err, Result};

/// Without an `identity_provider` and without native auth, `well_known.client`
/// alone must not bring up the OIDC server.
#[test]
fn oidc_absent_without_idp_or_native() -> Result {
	let db_path = format!("/tmp/tuwunel-test-native-gate-{}", process_id());

	let mut args = Args::default_test(&["fresh", "cleanup"]);
	args.maintenance = true;
	args.option.extend([
		format!("database_path=\"{db_path}\""),
		"well_known.client=\"https://localhost\"".to_owned(),
	]);

	let runtime = Runtime::new(Some(&args))?;
	let server = Server::new(Some(&args), Some(&runtime))?;

	let result: Result = runtime.block_on(async {
		let services = tuwunel::async_start(&server).await?;

		let outcome = match services.oauth.get_server() {
			| Ok(_) => Err!("OIDC server must not start from well_known.client alone"),
			| Err(_) => Ok(()),
		};

		server.server.shutdown()?;
		drop(services);

		tuwunel::async_run(&server).await?;
		tuwunel::async_stop(&server).await?;

		outcome
	});

	drop(runtime);

	remove_dir_all(&db_path).ok();

	result
}
