use std::fmt::Display;

#[derive(Debug)]
#[repr(i32)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    Encoding(&'static str),
    InvalidRequest(&'static str),
    Script(i8),
    Vm(String),
}

