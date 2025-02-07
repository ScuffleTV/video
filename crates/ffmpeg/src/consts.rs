pub(crate) const DEFAULT_BUFFER_SIZE: usize = 4096;

/// Const is an owned value which is immutable, but also has a lifetime.
/// This value exists because ffmpeg often has values that we 'own' but is linked
/// to some lifetime.
pub struct Const<'a, T>(pub(crate) T, pub(crate) std::marker::PhantomData<&'a ()>);

impl<T: std::fmt::Debug> std::fmt::Debug for Const<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> Const<'_, T> {
    pub(crate) const fn new(value: T) -> Self {
        Self(value, std::marker::PhantomData)
    }
}

impl<T> std::ops::Deref for Const<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Mut is an owned value which is mutable, but also has a lifetime.
/// This value exists because ffmpeg often has values that we 'own' but is linked
/// to some lifetime.
pub struct Mut<'a, T>(pub(crate) T, pub(crate) std::marker::PhantomData<&'a ()>);

impl<T: std::fmt::Debug> std::fmt::Debug for Mut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> Mut<'_, T> {
    pub(crate) const fn new(value: T) -> Self {
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

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use crate::consts::Mut;

    #[test]
    fn test_mut_fmt_vec() {
        let value = vec![1, 2, 3];
        let mut_value = Mut::new(value);

        assert_eq!(format!("{:?}", mut_value), "[1, 2, 3]");
    }

    #[test]
    fn test_deref_for_mut_with_complex_type() {
        let value = vec![1, 2, 3];
        let mut_value = Mut::new(value);
        let deref_value: &Vec<i32> = &mut_value;

        assert_eq!(deref_value, &vec![1, 2, 3], "Dereferencing Mut should return the inner value");
    }
}
