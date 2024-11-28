use std::borrow::Cow;
use std::sync::Arc;

use opentelemetry::{Array, StringValue, Value};

#[doc(hidden)]
/// A compiler trick to create a specialization for a type.
/// Its particularly useful when we using macros so that we can at compile time
/// specialize for an arbitrary input type. We want to specialize for specific
/// types that have a known conversion to a `Value`, otherwise use the
/// `Into<Value>` trait. Or if the type implements `std::fmt::Display` or
/// `std::fmt::Debug` we use that to convert to a `String` and then to a
/// `Value`.
pub struct SpecializeValue<T>(Option<T>);

impl<T> SpecializeValue<T> {
	pub fn new(value: T) -> Self {
		Self(Some(value))
	}

	#[inline]
	pub fn take(&mut self) -> T {
		// Safety: `self` is a `Some` value
		unsafe { self.0.take().unwrap_unchecked() }
	}
}

#[doc(hidden)]
pub trait Specialization: private::Sealed {
	fn take_value(&mut self) -> Option<Value>;
}

#[doc(hidden)]
mod private {
	use super::SpecializeValue;

	pub trait Sealed {}

	impl<T: Sealed> Sealed for &mut T {}
	impl<T> Sealed for SpecializeValue<T> {}
}

macro_rules! sealed {
    ($($t:ty),*) => {
        $(impl private::Sealed for $t {})*
    };
}

macro_rules! integer_specialization {
	($type:ty) => {
		impl Specialization for Option<$type> {
			#[inline]
			fn take_value(&mut self) -> Option<Value> {
				Some(Value::I64(self.take()? as i64))
			}
		}

		sealed!(Option<$type>);
	};
}

integer_specialization!(i32);
integer_specialization!(i64);
integer_specialization!(u32);
integer_specialization!(i16);
integer_specialization!(u16);
integer_specialization!(i8);
integer_specialization!(u8);

impl Specialization for Option<f32> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::F64(self.take()? as f64))
	}
}

sealed!(Option<f32>);

impl Specialization for Option<f64> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::F64(self.take()?))
	}
}

sealed!(Option<f64>);

impl Specialization for Option<bool> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Bool(self.take()?))
	}
}

sealed!(Option<bool>);

impl Specialization for Option<&'static str> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::String(self.take()?.into()))
	}
}

sealed!(Option<&'static str>);

impl Specialization for Option<StringValue> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::String(self.take()?))
	}
}

sealed!(Option<StringValue>);

impl Specialization for Option<String> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::String(self.take()?.into()))
	}
}

sealed!(Option<String>);

impl Specialization for Option<Arc<str>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::String(self.take()?.into()))
	}
}

sealed!(Option<Arc<str>>);

impl Specialization for Option<Cow<'static, str>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::String(self.take()?.into()))
	}
}

sealed!(Option<Cow<'static, str>>);

impl Specialization for Option<Value> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		self.take()
	}
}

sealed!(Option<Value>);

impl Specialization for Option<Array> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(self.take()?))
	}
}

sealed!(Option<Array>);

impl Specialization for Option<Vec<bool>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::Bool(self.take()?)))
	}
}

sealed!(Option<Vec<bool>>);

macro_rules! integer_vector_specialization {
	($type:ty) => {
		impl Specialization for Option<Vec<$type>> {
			#[inline]
			fn take_value(&mut self) -> Option<Value> {
				Some(Value::Array(Array::I64(
					self.take()?.into_iter().map(|i| i as i64).collect(),
				)))
			}
		}

		sealed!(Option<Vec<$type>>);
	};
}

integer_vector_specialization!(i32);
integer_vector_specialization!(i64);
integer_vector_specialization!(u32);
integer_vector_specialization!(i16);
integer_vector_specialization!(u16);
integer_vector_specialization!(i8);
integer_vector_specialization!(u8);

impl Specialization for Option<Vec<f64>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::F64(self.take()?)))
	}
}

sealed!(Option<Vec<f64>>);

impl Specialization for Option<Vec<f32>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::F64(self.take()?.into_iter().map(|f| f as f64).collect())))
	}
}

sealed!(Option<Vec<f32>>);

impl Specialization for Option<Vec<&'static str>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::String(
			self.take()?.into_iter().map(|s| s.into()).collect(),
		)))
	}
}

sealed!(Option<Vec<&'static str>>);

impl Specialization for Option<Vec<StringValue>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::String(self.take()?)))
	}
}

sealed!(Option<Vec<StringValue>>);

impl Specialization for Option<Vec<String>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::String(
			self.take()?.into_iter().map(|s| s.into()).collect(),
		)))
	}
}

sealed!(Option<Vec<String>>);

impl Specialization for Option<Vec<Arc<str>>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::String(
			self.take()?.into_iter().map(|s| s.into()).collect(),
		)))
	}
}

sealed!(Option<Vec<Arc<str>>>);

impl Specialization for Option<Vec<Cow<'static, str>>> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::Array(Array::String(
			self.take()?.into_iter().map(|s| s.into()).collect(),
		)))
	}
}

sealed!(Option<Vec<Cow<'static, str>>>);

impl<T: std::fmt::Display> Specialization for &mut &mut SpecializeValue<T> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(Value::String(self.take().to_string().into()))
	}
}

impl<T: Into<Value>> Specialization for &mut &mut &mut SpecializeValue<T> {
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(self.take().into())
	}
}

impl<T> Specialization for &mut &mut &mut &mut SpecializeValue<T>
where
	T: Specialization,
{
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		self.take().take_value()
	}
}

impl<T> Specialization for &mut &mut &mut &mut &mut SpecializeValue<T>
where
	Option<T>: Specialization,
{
	#[inline]
	fn take_value(&mut self) -> Option<Value> {
		Some(self.take()).take_value()
	}
}

#[doc(hidden)]
#[macro_export]
macro_rules! to_value {
	($value:expr) => {{
		use $crate::value::Specialization;
		(&mut &mut &mut &mut &mut &mut $crate::value::SpecializeValue::new($value)).take_value()
	}};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_specialization_i64() {
		let value = to_value!(1);
		assert_eq!(value, Some(Value::I64(1)));
	}

	#[test]
	fn test_specialization_display() {
		struct Displayable(i64);

		impl std::fmt::Display for Displayable {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				write!(f, "{}", self.0)
			}
		}

		let value = to_value!(Displayable(1));
		assert_eq!(value, Some(Value::String("1".into())));
	}

	#[test]
	fn test_specialization_into() {
		struct Intoable(i64);

		impl From<Intoable> for Value {
			fn from(val: Intoable) -> Self {
				Value::I64(val.0)
			}
		}

		let value = to_value!(Intoable(1));
		assert_eq!(value, Some(Value::I64(1)));
	}

	#[test]
	fn test_specialization_array() {
		let value = to_value!(vec![1, 2, 3]);
		assert_eq!(value, Some(Value::Array(Array::I64(vec![1, 2, 3]))));
	}

	#[test]
	fn test_specialization_integer_vector_option() {
		let value = to_value!(Some(vec![1, 2, 3]));
		assert_eq!(value, Some(Value::Array(Array::I64(vec![1, 2, 3]))));
	}

	#[test]
	fn test_specialization_integer_vector_none() {
		let value = to_value!(None::<Vec<i32>>);
		assert_eq!(value, None);
	}

	#[test]
	fn test_specialization_f64_vector() {
		let value = to_value!(vec![1.0, 2.0, 3.0]);
		assert_eq!(value, Some(Value::Array(Array::F64(vec![1.0, 2.0, 3.0]))));
	}

	#[test]
	fn test_specialization_f32_vector() {
		let value = to_value!(vec![1.0f32, 2.0f32, 3.0f32]);
		assert_eq!(value, Some(Value::Array(Array::F64(vec![1.0, 2.0, 3.0]))));
	}

	#[test]
	fn test_none_str() {
		let value = to_value!(None::<&'static str>);
		assert_eq!(value, None);
	}

	#[test]
	fn test_some_str() {
		let value = to_value!("hello");
		assert_eq!(value, Some(Value::String("hello".into())));
	}
}
