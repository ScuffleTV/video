macro_rules! from_error {
	($tt:ty, $val:expr, $err:ty) => {
		impl From<$err> for $tt {
			fn from(error: $err) -> Self {
				$val(error)
			}
		}
	};
}

pub(super) use from_error;
