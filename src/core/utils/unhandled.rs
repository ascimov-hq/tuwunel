//! Macros for branches expected never to execute.
//!
//! Active builds expand to `unimplemented!()`. A dormant
//! `unreachable_unchecked()` definition is excluded by `#[cfg(disable)]`;
//! activating it would also require excluding the ordinary definition.

#[cfg(disable)] // activate when more stable and callsites are vetted.
// #[cfg(not(debug_assertions))]
/// Defines a dormant unchecked marker for branches assumed impossible.
///
/// This definition is excluded by `cfg(disable)`, and activating it requires
/// also excluding the safe definition below. Reaching its expansion invokes
/// [`std::hint::unreachable_unchecked`] and causes undefined behavior.
#[macro_export]
macro_rules! unhandled {
	($msg:literal) => {
		// SAFETY: Eliminates branches never encountered in the codebase. This can
		// promote optimization and reduce codegen. The developer must verify for every
		// invoking callsite that the unhandled type is in no way involved and could not
		// possibly be encountered.
		unsafe {
			std::hint::unreachable_unchecked();
		}
	};
}

//#[cfg(debug_assertions)]
/// Marks an unsupported branch and panics with the supplied message.
///
/// The expansion delegates to [`crate::maybe_unhandled!`] and always retains a
/// runtime failure path.
#[macro_export]
macro_rules! unhandled {
	($msg:literal) => {
		$crate::maybe_unhandled!($msg)
	};
}

/// Panics with the supplied message for a branch that is not implemented.
///
/// This macro always retains a runtime failure path and can therefore mark code
/// that may remain reachable.
#[macro_export]
macro_rules! maybe_unhandled {
	($msg:literal) => {
		unimplemented!($msg)
	};
}
