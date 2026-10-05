pub mod dump;

use std::sync::{
	Arc, Mutex,
	atomic::{AtomicU32, AtomicU64},
};
#[cfg(tokio_unstable)]
use std::{iter::repeat, ops::Range, time::Duration};

#[cfg(tokio_unstable)]
use smallvec::SmallVec;
use tokio::runtime;
#[cfg(tokio_unstable)]
use tokio_metrics::{RuntimeIntervals, RuntimeMonitor};
use tokio_metrics::{TaskMetrics, TaskMonitor};

/// Bucket counts sampled from the scheduler latency histogram.
///
/// The inline budget matches the default bucket count; a runtime configured
/// with more buckets spills to the heap.
#[cfg(tokio_unstable)]
type Counts = SmallVec<[u64; 20]>;

/// One bucket of the scheduler latency histogram.
///
/// The range is the bucket's latency span as configured on the runtime; the
/// count is how many task schedules landed within it.
#[cfg(tokio_unstable)]
type Bucket = (Range<Duration>, u64);

pub struct Metrics {
	_runtime: Option<runtime::Handle>,

	runtime_metrics: Option<runtime::RuntimeMetrics>,

	task_monitor: Option<TaskMonitor>,

	task_intervals: Mutex<Option<Box<dyn Iterator<Item = TaskMetrics> + Send>>>,

	#[cfg(tokio_unstable)]
	_runtime_monitor: Option<RuntimeMonitor>,

	#[cfg(tokio_unstable)]
	runtime_intervals: Mutex<Option<RuntimeIntervals>>,

	#[cfg(tokio_unstable)]
	sched_histogram_last: Mutex<Counts>,

	// TODO: move stats
	pub requests_count: AtomicU64,
	pub requests_handle_finished: AtomicU64,
	pub requests_handle_active: AtomicU32,
	pub requests_panic: AtomicU32,
}

impl Metrics {
	#[must_use]
	pub fn new(runtime: Option<&runtime::Handle>) -> Arc<Self> {
		#[cfg(tokio_unstable)]
		let runtime_monitor = runtime.map(RuntimeMonitor::new);

		#[cfg(tokio_unstable)]
		let runtime_intervals = runtime_monitor
			.as_ref()
			.map(RuntimeMonitor::intervals);

		let task_monitor = cfg!(tokio_unstable).then(|| {
			TaskMonitor::builder()
				.with_slow_poll_threshold(TaskMonitor::DEFAULT_SLOW_POLL_THRESHOLD)
				.with_long_delay_threshold(TaskMonitor::DEFAULT_LONG_DELAY_THRESHOLD)
				.clone()
				.build()
		});

		let task_intervals = task_monitor.as_ref().map(
			|task_monitor| -> Box<dyn Iterator<Item = TaskMetrics> + Send> {
				Box::new(task_monitor.intervals())
			},
		);

		Arc::new(Self {
			_runtime: runtime.cloned(),

			runtime_metrics: runtime.map(runtime::Handle::metrics),

			task_monitor,

			task_intervals: task_intervals.into(),

			#[cfg(tokio_unstable)]
			_runtime_monitor: runtime_monitor,

			#[cfg(tokio_unstable)]
			runtime_intervals: Mutex::new(runtime_intervals),

			#[cfg(tokio_unstable)]
			sched_histogram_last: Counts::new().into(),

			requests_count: AtomicU64::new(0),
			requests_handle_finished: AtomicU64::new(0),
			requests_handle_active: AtomicU32::new(0),
			requests_panic: AtomicU32::new(0),
		})
	}

	#[inline]
	pub async fn instrument<F, Output>(&self, f: F) -> Output
	where
		F: Future<Output = Output>,
	{
		if let Some(monitor) = self.task_metrics() {
			monitor.instrument(f).await
		} else {
			f.await
		}
	}

	pub fn task_interval(&self) -> Option<TaskMetrics> {
		self.task_intervals
			.lock()
			.expect("locked")
			.as_mut()
			.and_then(Iterator::next)
	}

	#[cfg(tokio_unstable)]
	pub fn runtime_interval(&self) -> Option<tokio_metrics::RuntimeMetrics> {
		self.runtime_intervals
			.lock()
			.expect("locked")
			.as_mut()
			.map(Iterator::next)
			.expect("next interval")
	}

	/// Bucket deltas of the scheduler latency histogram since the last call.
	///
	/// Each call diffs the runtime's cumulative per-worker counts against the
	/// previous sample and replaces it, so the counts are the traffic of one
	/// interval rather than the totals tokio exposes. Returns `None` when the
	/// runtime was built without the histogram.
	#[cfg(tokio_unstable)]
	pub fn sched_histogram_interval(&self) -> Option<impl Iterator<Item = Bucket>> {
		let metrics = self
			.runtime_metrics()
			.filter(|metrics| metrics.schedule_latency_histogram_enabled())?;

		let num_workers = metrics.num_workers();
		let bucket_total = |bucket| -> u64 {
			(0..num_workers)
				.map(|worker| metrics.schedule_latency_histogram_bucket_count(worker, bucket))
				.sum()
		};

		let num_buckets = metrics.schedule_latency_histogram_num_buckets();
		let totals: Counts = (0..num_buckets).map(bucket_total).collect();

		let mut last = self.sched_histogram_last.lock().expect("locked");
		let deltas: Counts = totals
			.iter()
			.zip(last.iter().copied().chain(repeat(0)))
			.map(|(total, last)| total.saturating_sub(last))
			.collect();

		*last = totals;

		let ranges = (0..num_buckets)
			.map(move |bucket| metrics.schedule_latency_histogram_bucket_range(bucket));

		Some(ranges.zip(deltas))
	}

	#[inline]
	pub fn num_workers(&self) -> usize {
		self.runtime_metrics()
			.map_or(0, runtime::RuntimeMetrics::num_workers)
	}

	#[inline]
	pub fn task_metrics(&self) -> Option<&TaskMonitor> { self.task_monitor.as_ref() }

	#[inline]
	pub fn runtime_metrics(&self) -> Option<&runtime::RuntimeMetrics> {
		self.runtime_metrics.as_ref()
	}
}
