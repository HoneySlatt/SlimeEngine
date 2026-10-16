//! Persistent identities: how files and saves name an entity.

use std::fmt;
use std::str::FromStr;

/// How files and saves name an entity (`docs/adr/0079-the-engines-own-world.md`): the path of
/// the IDs local to each file through the scenes that placed it, from the key the world spawned
/// the outermost one under. The node `door` of a scene spawned under the key `zone` is `zone/door`.
///
/// An entity that a world creates outside a scene ([`World::spawn`](crate::World::spawn)) is
/// `created/<number>`, from a count that saves keep. No part holds `/`, nor is empty: `Display`
/// joins the parts with `/`, and parsing splits the text there.
///
/// An entity's [`Entity`](crate::Entity) handle changes from one run to the next; its persistent
/// identity does not, as long as the names it is made of stay.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PersistentId(Vec<String>);

impl PersistentId {
    /// The identity made of these parts, outermost first.
    ///
    /// # Panics
    ///
    /// If there are no parts, or a part is empty or holds `/`.
    pub fn new<S: Into<String>>(parts: impl IntoIterator<Item = S>) -> Self {
        let parts: Vec<String> = parts.into_iter().map(Into::into).collect();
        assert!(!parts.is_empty(), "a persistent identity has parts");
        parts.iter().for_each(|part| check_part(part));
        Self(parts)
    }

    /// The identity of what a scene placed under this one: this path and `local`.
    ///
    /// # Panics
    ///
    /// If `local` is empty or holds `/`.
    pub fn child(&self, local: impl Into<String>) -> Self {
        let local = local.into();
        check_part(&local);
        let mut parts = self.0.clone();
        parts.push(local);
        Self(parts)
    }

    /// The path's parts, outermost first.
    pub fn parts(&self) -> &[String] {
        &self.0
    }

    /// The identity of the entity created outside a scene under `number`.
    pub(crate) fn created(number: u64) -> Self {
        Self(vec![CREATED.to_owned(), number.to_string()])
    }

    /// The number of the entity created outside a scene that this identity names, if it names one.
    pub(crate) fn created_number(&self) -> Option<u64> {
        match self.0.as_slice() {
            [key, number] if key == CREATED => number.parse().ok(),
            _ => None,
        }
    }
}

/// Panics if `part` cannot be a part of an identity: empty, or holding `/`.
fn check_part(part: &str) {
    assert!(
        !part.is_empty() && !part.contains('/'),
        "`{part}` cannot be a part of a persistent identity: it is empty or holds `/`"
    );
}

/// The key under which a world names the entities it creates outside scenes, which no scene is
/// spawned under.
pub(crate) const CREATED: &str = "created";

/// Text that is not a persistent identity: an empty part, as `zone//door` or `` give.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a persistent identity: its parts, between `/`, are not empty")]
pub struct ParsePersistentIdError(String);

/// The identity whose parts `text` joins with `/`.
impl FromStr for PersistentId {
    type Err = ParsePersistentIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let parts: Vec<String> = text.split('/').map(str::to_owned).collect();
        if parts.iter().any(String::is_empty) {
            return Err(ParsePersistentIdError(text.to_owned()));
        }
        Ok(Self(parts))
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
