use std::sync::Arc;

use figment::Figment;
use tracing::subscriber::NoSubscriber;

use super::Server;
use crate::{
	config::{Config, Sources},
	log::{LogLevelReloadHandles, Logging, capture::State},
	metrics::Metrics,
};

fn server() -> Server {
	let raw = Figment::new().merge(("server_name", "test.example"));
	let config = Config::new(&raw).expect("minimal config");

	let log = Logging {
		subscriber: Arc::new(NoSubscriber::new()),
		reload: LogLevelReloadHandles::default(),
		capture: Arc::new(State::new()),
	};

	Server::new(config, Sources::default(), None, log, Metrics::new(None))
}

#[test]
fn a_restore_is_claimed_once() {
	let server = server();

	assert!(server.claim_backup_restore());
	assert!(!server.claim_backup_restore());
	assert!(!server.claim_backup_restore());
}
