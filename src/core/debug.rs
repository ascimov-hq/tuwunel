//! Provides diagnostics, debugger traps, and configurable debug logging.
//!
//! Tracing macros retain their requested levels when extra diagnostics are
//! enabled and otherwise demote events to `DEBUG`. The module also re-exports
//! conditional result inspection and bounded formatting helpers for diagnostic
//! paths.

use std::{any::Any, env, panic, sync::LazyLock};

use tracing::Level;
/// Reports an annotated item's parsed syntax-tree depth and length during
/// compilation.
///
/// Expansion prints the greatest indentation depth and formatted tree line
/// count. The annotated item is returned unchanged.
pub use tuwunel_macros::recursion_depth;

/// Provides conditional inspection methods for `Result` values.
///
/// Debug-assertion builds invoke a closure on the contained success or error
/// value. Other builds return the result unchanged without invoking the
/// closure.
pub use crate::result::DebugInspect;
/// Provides diagnostic formatting adapters.
///
/// These adapters limit slice or string output for tracing fields and other
/// debug-oriented diagnostics.
pub use crate::utils::debug::*;

/// Emits a tracing event with debug-aware level control.
///
/// When [`logging`] is true, the requested level is retained and `_debug =
/// true` is attached. Otherwise the event is emitted at `DEBUG`, allowing
/// compile-time filters to remove it.
#[macro_export]
#[collapse_debuginfo(yes)]
macro_rules! debug_event {
	( $level:expr_2021, $($x:tt)+ ) => {
		if $crate::debug::logging() {
			::tracing::event!( $level, _debug = true, $($x)+ )
		} else {
			::tracing::debug!( $($x)+ )
		}
	}
}

/// Emits an error event through debug-aware level control.
///
/// When extra debug logging is disabled, the event is demoted to `DEBUG` and
/// may be removed by compile-time filtering.
#[macro_export]
macro_rules! debug_error {
	( $($x:tt)+ ) => {
		$crate::debug_event!(::tracing::Level::ERROR, $($x)+ )
	}
}

/// Emits a warning event through debug-aware level control.
///
/// When extra debug logging is disabled, the event is demoted to `DEBUG` and
/// may be removed by compile-time filtering.
#[macro_export]
macro_rules! debug_warn {
	( $($x:tt)+ ) => {
		$crate::debug_event!(::tracing::Level::WARN, $($x)+ )
	}
}

/// Emits an informational event through debug-aware level control.
///
/// When extra debug logging is disabled, the event is demoted to `DEBUG` and
/// may be removed by compile-time filtering.
#[macro_export]
macro_rules! debug_info {
	( $($x:tt)+ ) => {
		$crate::debug_event!(::tracing::Level::INFO, $($x)+ )
	}
}

/// Selects `INFO` or `DEBUG` for diagnostic tracing spans.
///
/// The level is `INFO` when extra diagnostics retain their requested levels and
/// `DEBUG` otherwise. Release filters can therefore elide these spans alongside
/// the debug event macros.
pub const INFO_SPAN_LEVEL: Level = if logging() { Level::INFO } else { Level::DEBUG };

/// Reports whether the process environment suggests a `gdb` launch.
///
/// An `_` environment value ending in `gdb` is treated as a debugger launch.
/// Missing or non-Unicode values produce `false`.
pub static DEBUGGER: LazyLock<bool> =
	LazyLock::new(|| env::var("_").unwrap_or_default().ends_with("gdb"));

#[cfg_attr(debug_assertions, crate::ctor(unsafe))]
#[cfg_attr(not(debug_assertions), expect(dead_code))]
fn set_panic_trap() {
	if !*DEBUGGER {
		return;
	}

	let next = panic::take_hook();
	panic::set_hook(Box::new(move |info| {
		panic_handler(info, &next);
	}));
}

/// Invokes a debugger trap before forwarding a panic to another hook.
///
/// If the trap returns, the supplied hook receives the original panic
/// information. Supported targets can therefore stop in a debugger before
/// normal panic reporting continues.
#[cold]
#[inline(never)]
pub fn panic_handler(info: &panic::PanicHookInfo<'_>, next: &dyn Fn(&panic::PanicHookInfo<'_>)) {
	trap();
	next(info);
}

/// Raises a debugger breakpoint on supported build targets.
///
/// Builds with `core_intrinsics` use the compiler breakpoint intrinsic, while
/// `x86_64` builds use `int3` as a fallback. Other targets perform no operation
/// without intrinsic support.
#[inline(always)]
pub fn trap() {
	#[cfg(core_intrinsics)]
	//SAFETY: embeds llvm intrinsic for hardware breakpoint
	unsafe {
		std::intrinsics::breakpoint();
	}

	#[cfg(all(not(core_intrinsics), target_arch = "x86_64"))]
	//SAFETY: embeds instruction for hardware breakpoint
	unsafe {
		std::arch::asm!("int3");
	}
}

/// Extracts a static string slice from a boxed panic payload.
///
/// Payloads of other types, including owned `String` values, produce the empty
/// string. The returned slice does not borrow storage from the box.
#[must_use]
pub fn panic_str(p: &Box<dyn Any + Send + 'static>) -> &'static str {
	(**p)
		.downcast_ref::<&str>()
		.copied()
		.unwrap_or_default()
}

/// Returns the compiler-generated name of an argument's statically inferred
/// type.
///
/// The value is used only to infer the generic type and is not inspected. The
/// returned name is intended for diagnostics and its format is not stable.
#[inline(always)]
#[must_use]
pub fn rttype_name<T: ?Sized>(_: &T) -> &'static str { type_name::<T>() }

/// Returns the compiler-generated name of a generic type.
///
/// The name is intended for diagnostics rather than program logic. Its exact
/// format can change between compiler versions.
#[inline(always)]
#[must_use]
pub fn type_name<T: ?Sized>() -> &'static str { std::any::type_name::<T>() }

/// Returns whether extra logging calls retain their requested levels.
///
/// Debug-assertion builds, `tuwunel_debug_logging`, or an absent
/// `release_max_log_level` feature enable the extra levels. When disabled,
/// callers demote these events to `DEBUG` so compile-time filtering can remove
/// them.
#[must_use]
#[inline]
pub const fn logging() -> bool {
	cfg!(debug_assertions)
		|| cfg!(tuwunel_debug_logging)
		|| !cfg!(feature = "release_max_log_level")
}
