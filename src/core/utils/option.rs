//! Asynchronous mapping adapters for optional values.
//!
//! [`OptionExt`] turns an optional value into an optional future or a
//! zero-or-one stream. The mapping closure runs only when the option contains a
//! value.

use futures::{FutureExt, Stream, future::OptionFuture};

use super::IterStream;

/// Extends [`Option`] with asynchronous mapping adapters.
///
/// [`OptionExt::map_async`] preserves optionality through future completion.
/// [`OptionExt::map_stream`] exposes the mapped result as a zero-or-one stream.
pub trait OptionExt<T> {
	/// Maps the contained value to a future without eagerly awaiting it.
	///
	/// A [`Some`] value yields an [`OptionFuture`] that resolves to the mapped
	/// output. [`None`] resolves to `None` and never calls the mapping
	/// closure.
	fn map_async<F, Fut, U>(self, f: F) -> OptionFuture<Fut>
	where
		F: FnOnce(T) -> Fut,
		Fut: Future<Output = U> + Send,
		U: Send;

	/// Maps the contained value to a future-backed stream with at most one
	/// item.
	///
	/// A [`Some`] value yields the mapped output after its future completes.
	/// [`None`] yields an empty stream and never calls the mapping closure.
	#[inline]
	fn map_stream<F, Fut, U>(self, f: F) -> impl Stream<Item = U> + Send
	where
		F: FnOnce(T) -> Fut,
		Fut: Future<Output = U> + Send,
		U: Send,
		Self: Sized,
	{
		self.map_async(f)
			.map(Option::into_iter)
			.map(IterStream::stream)
			.flatten_stream()
	}
}

impl<T> OptionExt<T> for Option<T> {
	#[inline]
	fn map_async<F, Fut, U>(self, f: F) -> OptionFuture<Fut>
	where
		F: FnOnce(T) -> Fut,
		Fut: Future<Output = U> + Send,
		U: Send,
	{
		self.map(f).into()
	}
}
