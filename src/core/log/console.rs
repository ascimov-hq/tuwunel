use std::{
	env, io,
	io::{IsTerminal, stdin},
	sync::LazyLock,
};

use tracing::{
	Event, Level, Metadata, Subscriber,
	field::{Field, Visit},
};
use tracing_subscriber::{
	field::RecordFields,
	fmt,
	fmt::{
		FmtContext, FormatEvent, FormatFields, MakeWriter,
		format::{Compact, DefaultVisitor, Format, Full, Pretty, Writer},
	},
	registry::LookupSpan,
};

use super::journald::{Entry, Journal, enabled as journald_enabled};
use crate::{Config, Result, apply, debug, is_equal_to};

static SYSTEMD_MODE: LazyLock<bool> =
	LazyLock::new(|| env::var("SYSTEMD_EXEC_PID").is_ok() && env::var("JOURNAL_STREAM").is_ok());

static TERMINAL_MODE: LazyLock<bool> = LazyLock::new(|| stdin().is_terminal());

pub struct ConsoleWriter {
	stdout: io::Stdout,
	stderr: io::Stderr,
	_journal_stream: [u64; 2],
	use_stderr: bool,
	journal: Option<Journal>,
}

pub enum Sink<'a> {
	Console(&'a ConsoleWriter),
	Journal(Entry<'a>),
}

impl ConsoleWriter {
	#[must_use]
	pub fn new(config: &Config) -> Self {
		let journal_stream = get_journal_stream();

		Self {
			stdout: io::stdout(),
			stderr: io::stderr(),
			_journal_stream: journal_stream.into(),
			use_stderr: journal_stream.0 != 0 || config.log_to_stderr,
			journal: Journal::open(config),
		}
	}
}

impl<'a> MakeWriter<'a> for ConsoleWriter {
	type Writer = Sink<'a>;

	fn make_writer(&'a self) -> Self::Writer { Sink::Console(self) }

	fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
		self.journal
			.as_ref()
			.map_or(Sink::Console(self), |journal| Sink::Journal(journal.entry(meta)))
	}
}

impl io::Write for Sink<'_> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match self {
			| Self::Console(console) => console.write(buf),
			| Self::Journal(entry) => entry.write(buf),
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		match self {
			| Self::Console(console) => console.flush(),
			| Self::Journal(entry) => entry.flush(),
		}
	}
}

impl io::Write for &'_ ConsoleWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		if self.use_stderr {
			self.stderr.lock().write(buf)
		} else {
			self.stdout.lock().write(buf)
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		if self.use_stderr {
			self.stderr.lock().flush()
		} else {
			self.stdout.lock().flush()
		}
	}
}

pub struct ConsoleFormat {
	pretty: Format<Pretty>,
	full: Format<Full>,
	compact: Format<Compact>,
	compact_mode: bool,
}

impl ConsoleFormat {
	#[must_use]
	pub fn new(config: &Config) -> Self {
		let ansi = ansi_enabled(config);

		Self {
			pretty: fmt::format()
				.pretty()
				.with_ansi(ansi)
				.with_thread_names(true)
				.with_thread_ids(true)
				.with_target(true)
				.with_file(true)
				.with_line_number(true)
				.with_source_location(true),

			full: Format::<Full>::default()
				.with_thread_ids(config.log_thread_ids)
				.with_ansi(ansi),

			compact: fmt::format().compact().with_ansi(ansi),

			compact_mode: config.log_compact,
		}
	}
}

impl<S, N> FormatEvent<S, N> for ConsoleFormat
where
	S: Subscriber + for<'a> LookupSpan<'a>,
	N: for<'a> FormatFields<'a> + 'static,
{
	fn format_event(
		&self,
		ctx: &FmtContext<'_, S, N>,
		writer: Writer<'_>,
		event: &Event<'_>,
	) -> Result<(), std::fmt::Error> {
		let is_debug = debug::logging()
			&& event
				.fields()
				.map(|field| field.name())
				.any(is_equal_to!("_debug"));

		match *event.metadata().level() {
			| _ if self.compact_mode => self.compact.format_event(ctx, writer, event),
			| Level::ERROR if !is_debug => self.pretty.format_event(ctx, writer, event),
			| _ => self.full.format_event(ctx, writer, event),
		}
	}
}

struct ConsoleVisitor<'a> {
	visitor: DefaultVisitor<'a>,
}

impl<'writer> FormatFields<'writer> for ConsoleFormat {
	fn format_fields<R>(&self, writer: Writer<'writer>, fields: R) -> Result<(), std::fmt::Error>
	where
		R: RecordFields,
	{
		let mut visitor = ConsoleVisitor {
			visitor: DefaultVisitor::<'_>::new(writer, true),
		};

		fields.record(&mut visitor);

		Ok(())
	}
}

impl Visit for ConsoleVisitor<'_> {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		if field.name().starts_with('_') {
			return;
		}

		self.visitor.record_debug(field, value);
	}
}

#[must_use]
fn get_journal_stream() -> (u64, u64) {
	is_systemd_mode()
		.then(|| env::var("JOURNAL_STREAM").ok())
		.flatten()
		.as_deref()
		.and_then(|s| s.split_once(':'))
		.map(apply!(2, str::parse))
		.map(apply!(2, Result::unwrap_or_default))
		.unwrap_or((0, 0))
}

/// Whether to color the formatted line.
///
/// The journal takes that line verbatim and classifies a message carrying
/// control bytes as binary rather than text, so colors are suppressed while
/// entries are submitted to it.
#[inline]
#[must_use]
pub fn ansi_enabled(config: &Config) -> bool { config.log_colors && !journald_enabled(config) }

/// Whether the process was started by systemd, sampled once.
///
/// Both `SYSTEMD_EXEC_PID` and `JOURNAL_STREAM` have to be present, which the
/// service manager sets for a unit it launched itself.
#[inline]
#[must_use]
pub fn is_systemd_mode() -> bool { *SYSTEMD_MODE }

/// Whether standard input is attached to a terminal, sampled once.
#[inline]
#[must_use]
pub fn is_terminal_mode() -> bool { *TERMINAL_MODE }
