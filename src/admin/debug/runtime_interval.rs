#[cfg(tokio_unstable)]
use futures::TryStreamExt;
use tuwunel_core::Result;
#[cfg(tokio_unstable)]
use tuwunel_core::utils::stream::IterStream;

use crate::admin_command;

#[cfg(tokio_unstable)]
#[admin_command]
pub(super) async fn runtime_interval(&self) -> Result {
	let metrics = &self.services.server.metrics;

	let Some(interval) = metrics.runtime_interval() else {
		return self
			.write_str("Runtime metrics are not available.")
			.await;
	};

	writeln!(self, "```rs\n{interval:#?}\nschedule_latency_histogram: [").await?;

	metrics
		.sched_histogram_interval()
		.into_iter()
		.flatten()
		.try_stream()
		.try_for_each(|(range, count)| writeln!(self, "    {range:?}: {count},"))
		.await?;

	self.write_str("]\n```").await
}

#[cfg(not(tokio_unstable))]
#[admin_command]
pub(super) async fn runtime_interval(&self) -> Result {
	self.write_str("Runtime metrics require building with `tokio_unstable`.")
		.await
}
