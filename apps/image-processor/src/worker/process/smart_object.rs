use std::ptr::NonNull;

pub type SmartPtr<T> = SmartObject<NonNull<T>>;

#[derive(Debug)]
pub struct SmartObject<T> {
	value: Option<T>,
	destructor: fn(&mut T),
}

impl<T> SmartObject<T> {
	pub fn new(value: T, destructor: fn(&mut T)) -> Self {
		Self {
			value: Some(value),
			destructor,
		}
	}

	pub fn free(mut self) -> T {
		self.destructor = |_| {};
		self.value.take().unwrap()
	}
}

impl<T> Drop for SmartObject<T> {
	fn drop(&mut self) {
		if let Some(mut value) = self.value.take() {
			(self.destructor)(&mut value);
		}
	}
}

impl<T> std::ops::Deref for SmartObject<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.value.as_ref().unwrap()
	}
}

impl<T> std::ops::DerefMut for SmartObject<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.value.as_mut().unwrap()
	}
}

impl<T> AsRef<T> for SmartPtr<T> {
	fn as_ref(&self) -> &T {
		unsafe { self.value.unwrap().as_ref() }
	}
}

impl<T> AsMut<T> for SmartPtr<T> {
	fn as_mut(&mut self) -> &mut T {
		unsafe { self.value.unwrap().as_mut() }
	}
}
