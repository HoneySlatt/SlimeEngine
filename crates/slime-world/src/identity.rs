//! Persistent identities: how files and saves name an entity.

use std::fmt;

/// How files and saves name an entity (`docs/adr/0079-the-engines-own-world.md`): the path of
/// the IDs local to each file through the scenes that placed it, from the key the world spawned
/// the outermost one under. The node `door` of a scene spawned under the key `zone` is `zone/door`.
///
/// An entity's [`Entity`](crate::Entity) handle changes from one run to the next; its persistent
/// identity does not, as long as the names it is made of stay. `Display` joins the path's parts
/// with `/`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PersistentId(Vec<String>);

impl PersistentId {
    /// The identity made of these parts, outermost first.
    ///
    /// # Panics
    ///
    /// If there are no parts.
    pub fn new<S: Into<String>>(parts: impl IntoIterator<Item = S>) -> Self {
        let parts: Vec<String> = parts.into_iter().map(Into::into).collect();
        assert!(!parts.is_empty(), "a persistent identity has parts");
        Self(parts)
    }

    /// The identity of what a scene placed under this one: this path and `local`.
    pub fn child(&self, local: impl Into<String>) -> Self {
        let mut parts = self.0.clone();
        parts.push(local.into());
        Self(parts)
    }

    /// The path's parts, outermost first.
    pub fn parts(&self) -> &[String] {
        &self.0
    }
}

/// The parts joined by `/`: `zone/door`.
impl fmt::Display for PersistentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_prints_its_path() {
        let zone = PersistentId::new(["zone"]);
        assert_eq!(zone.child("door").to_string(), "zone/door");
        assert_eq!(zone.child("door").parts(), ["zone", "door"]);
    }
}
