//! Scope-exit guards for cleanup and temporary state changes.
//!
//! The macros create local drop guards that run cleanup as control leaves the
//! surrounding scope. They cover arbitrary deferred actions and restoration of
//! a replaced value.

/// Runs an action when the surrounding scope exits.
///
/// A local drop guard executes the action on normal return, early return, or
/// unwinding. Multiple guards in one scope run in reverse declaration order.
#[macro_export]
macro_rules! defer {
	($body:block) => {
		struct _Defer_<F: FnMut()> {
			closure: F,
		}

		impl<F: FnMut()> Drop for _Defer_<F> {
			fn drop(&mut self) { (self.closure)(); }
		}

		let _defer_ = _Defer_ { closure: || $body };
	};

	($body:expr_2021) => {
		$crate::defer! {{ $body }}
	};
}

/// Temporarily replaces a value and restores the previous value at scope exit.
///
/// The first argument is a mutable-reference identifier, and the second becomes
/// its temporary value. A deferred drop guard performs restoration on normal
/// return, early return, or unwinding.
#[macro_export]
macro_rules! scope_restore {
	($val:ident, $ours:expr_2021) => {
		let theirs = $crate::utils::exchange($val, $ours);
		$crate::defer! {{ *$val = theirs; }};
	};
}
