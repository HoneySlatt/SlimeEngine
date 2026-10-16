#![doc = include_str!("../README.md")]

// The code that the derive writes names this crate `::slime_reflect`: its own tests derive too.
extern crate self as slime_reflect;

mod diff;
mod error;
mod impls;
mod info;
mod path;
mod reflect;
mod registry;
pub mod support;
mod value;

pub use diff::{apply, diff};
pub use error::{Problem, ReflectError, RegistryError};
pub use info::{FieldInfo, Kind, Range, TypeInfo, VariantInfo};
pub use path::{ParsePathError, Path, Step};
pub use reflect::Reflect;
pub use registry::{Leaf, Registration, Registry};
/// Derives [`Reflect`](trait@Reflect): its documentation says which attributes the derive takes,
/// and what it refuses.
pub use slime_reflect_derive::Reflect;
pub use value::Value;

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}

    #[test]
    fn values_registries_and_errors_can_be_shared_between_threads() {
        send_and_sync::<Value>();
        send_and_sync::<Registry>();
        send_and_sync::<ReflectError>();
    }
}
