//! The engine's families, each held to its version by its description
//! (`docs/adr/0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md`).
//! A change of their messages fails here until their description is written again, and their
//! version raised where `uwui_agent::description` says. The replies, which the three families
//! share, are described whole in each one's file.

use std::path::Path;

use schemars::{JsonSchema, schema_for};
use uwui_agent::Value;
use uwui_agent::description::{Description, Kind, WRITE_VARIABLE, check};

use crate::{EDITING, Edit, Event, FAMILIES, PLAY, Reply, Request, WORLD};

fn schema<T: JsonSchema>() -> Value {
    schema_for!(T).to_value()
}

/// The family `family`'s description: its requests, which its methods name, its events, and the
/// replies.
fn described(family: &str) -> Description {
    let ours = |method: &str| {
        method
            .split_once('/')
            .is_some_and(|(name, _)| name == family)
    };
    let mut description = Description::new();
    for requests in [schema::<Request>(), schema::<Edit>()] {
        description
            .tagged(Kind::Request, &requests, "method", Some("params"), ours)
            .expect("described");
    }
    description
        .tagged(
            Kind::Event,
            &schema::<Event>(),
            "method",
            Some("params"),
            ours,
        )
        .expect("described");
    description
        .tagged(Kind::Reply, &schema::<Reply>(), "reply", None, |_| true)
        .expect("described");
    description
}

fn held(family: &str) {
    let version = FAMILIES
        .iter()
        .find(|(name, ..)| *name == family)
        .map(|&(_, version, _)| version)
        .expect("a family of the channel's");
    let directory = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/families"));
    let write = std::env::var(WRITE_VARIABLE).is_ok_and(|value| value == "write");
    check(directory, family, version, &described(family), write)
        .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_family_of_play_is_described() {
    held(PLAY);
}

#[test]
fn the_family_of_the_world_is_described() {
    held(WORLD);
}

#[test]
fn the_family_of_editing_is_described() {
    held(EDITING);
}
