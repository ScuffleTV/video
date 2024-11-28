#![allow(clippy::all)]
#![allow(unused_imports)]
#![allow(unused_qualifications)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(unused_mut)]
#![allow(unused_parens)]

include!(concat!(env!("OUT_DIR"), "/module.rs"));

pub mod ext;
