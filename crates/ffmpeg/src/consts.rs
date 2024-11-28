pub(crate) const DEFAULT_BUFFER_SIZE: usize = 4096;

/// Const is a owned value which is immutable, but also has a lifetime.
pub struct Const<'a, T>(pub(crate) T, pub(crate) std::marker::PhantomData<&'a ()>);

impl<T: std::fmt::Debug> std::fmt::Debug for Const<'_, T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

impl<T> Const<'_, T> {
	pub(crate) fn new(value: T) -> Self {
		Self(value, std::marker::PhantomData)
	}
}

impl<T> std::ops::Deref for Const<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// Mut is a owned value which is mutable, but also has a lifetime.
pub struct Mut<'a, T>(pub(crate) T, pub(crate) std::marker::PhantomData<&'a ()>);

impl<T: std::fmt::Debug> std::fmt::Debug for Mut<'_, T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

impl<T> Mut<'_, T> {
	pub(crate) fn new(value: T) -> Self {
		Self(value, std::marker::PhantomData)
	}
}

impl<T> std::ops::Deref for Mut<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> std::ops::DerefMut for Mut<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
