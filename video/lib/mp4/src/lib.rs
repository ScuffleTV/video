mod boxes;

pub mod codec;

pub use boxes::{header, types, BoxType, DynBox};

#[cfg(test)]
mod tests;
