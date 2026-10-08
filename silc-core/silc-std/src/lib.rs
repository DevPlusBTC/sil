//! silc-std: Standard Library Prelude for SIL Language v0.1.0 (minimal)

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[macro_export]
macro_rules! bail { ($msg:expr) => { panic!("{}", $msg) }; }
#[macro_export]
macro_rules! ensure { ($cond:expr, $msg:expr) => { if !$cond { panic!("{}", $msg) } }; }

pub mod primitive {
    pub type Entero64 = i64;
    pub type Flotante64 = f64;
    pub type Booleano = bool;
    pub type Texto = String;
    pub type Byte = u8;
    pub type Void = ();

    pub trait SilPrimitive: Sized + Clone {
        fn to_sil_string(&self) -> String;
        fn from_sil_string(s: &str) -> Result<Self, &'static str>;
    }

    impl SilPrimitive for i64 {
        fn to_sil_string(&self) -> String { self.to_string() }
        fn from_sil_string(s: &str) -> Result<Self, &'static str> {
            s.parse().map_err(|_| "Invalid Entero64")
        }
    }

    impl SilPrimitive for f64 {
        fn to_sil_string(&self) -> String { self.to_string() }
        fn from_sil_string(s: &str) -> Result<Self, &'static str> {
            s.parse().map_err(|_| "Invalid Flotante64")
        }
    }

    impl SilPrimitive for bool {
        fn to_sil_string(&self) -> String { self.to_string() }
        fn from_sil_string(s: &str) -> Result<Self, &'static str> {
            match s {
                "verdadero" | "true" | "1" => Ok(true),
                "falso" | "false" | "0" => Ok(false),
                _ => Err("Invalid Booleano"),
            }
        }
    }

    impl SilPrimitive for String {
        fn to_sil_string(&self) -> String { self.clone() }
        fn from_sil_string(s: &str) -> Result<Self, &'static str> { Ok(s.to_string()) }
    }
}

pub mod error {
    #[derive(Debug, Clone)]
    pub struct SilError {
        msg: String,
    }

    impl SilError {
        pub fn new(msg: &str) -> Self {
            Self { msg: msg.to_string() }
        }
    }

    impl core::fmt::Display for SilError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}", self.msg)
        }
    }

    impl std::error::Error for SilError {}

    pub type SilResult<T> = Result<T, SilError>;
}

pub mod option {
    pub use core::option::Option::{self, Some, None};

    pub trait OptionExt<T> {
        fn unwrap_or(self, default: T) -> T;
        fn expect(self, msg: &str) -> T;
    }

    impl<T> OptionExt<T> for Option<T> {
        fn unwrap_or(self, default: T) -> T { self.unwrap_or(default) }
        fn expect(self, msg: &str) -> T { self.expect(msg) }
    }
}

pub mod result {
    pub use core::result::Result::{self, Ok, Err};

    pub trait ResultExt<T, E> {
        fn unwrap_or(self, default: T) -> T;
        fn map<U>(self, f: impl FnOnce(T) -> U) -> Result<U, E>;
    }

    impl<T, E> ResultExt<T, E> for Result<T, E> {
        fn unwrap_or(self, default: T) -> T { self.unwrap_or(default) }
        fn map<U>(self, f: impl FnOnce(T) -> U) -> Result<U, E> { self.map(f) }
    }
}

pub mod prelude {
    pub use crate::primitive::{Entero64, Flotante64, Booleano, Texto, Byte, Void, SilPrimitive};
    pub use crate::error::{SilError, SilResult};
    pub use crate::option::{Option, Some, None, OptionExt};
    pub use crate::result::{Result, Ok, Err, ResultExt};
    pub use crate::{bail, ensure};
}

pub use prelude::*;
pub use primitive::{Entero64, Flotante64, Booleano, Texto, Byte, Void, SilPrimitive};
pub use error::{SilError, SilResult};
pub use option::{Option, Some, None, OptionExt};
pub use result::{Result, Ok, Err, ResultExt};
