//! The core's messages and the interface's family, each held to its version by its description
//! (`docs/adr/0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md`).
//! A change of their messages fails here until their description is written again, and their
//! version raised where `crate::description` says.

use std::path::Path;

use schemars::{JsonSchema, schema_for};

use crate::description::{Description, Kind, WRITE_VARIABLE, check};
use crate::session::{Cancel, Subscription};
use crate::{CORE_VERSION, Fault, Hello, Welcome, interface};

/// Where the families' descriptions are kept.
fn directory() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/families"))
}

/// Whether the descriptions are to be written ([`WRITE_VARIABLE`]).
fn write() -> bool {
    std::env::var(WRITE_VARIABLE).is_ok_and(|value| value == "write")
}

fn schema<T: JsonSchema>() -> crate::Value {
    schema_for!(T).to_value()
}

#[test]
fn the_core_is_described() {
    let mut core = Description::new();
    core.message(Kind::Request, crate::HELLO, &schema::<Hello>())
        .expect("described");
    core.message(Kind::Reply, crate::HELLO, &schema::<Welcome>())
        .expect("described");
    core.message(Kind::Request, crate::CANCEL, &schema::<Cancel>())
        .expect("described");
    for subscription in [crate::SUBSCRIBE, crate::UNSUBSCRIBE] {
        core.message(
            Kind::Request,
            subscription,
            &schema::<Subscription<'static>>(),
        )
        .expect("described");
        core.line(&format!("reply {subscription} . object"));
    }
    // The envelope is written by two private types in `message.rs`, which no schema describes;
    // its error is a `Fault`, read from its type.
    for line in [
        "envelope message . object",
        "envelope message jsonrpc string const=\"2.0\"",
        "envelope message id integer|null optional",
        "envelope message method string optional",
        "envelope message params any optional",
        "envelope message result any optional",
        "envelope message error object optional",
        "envelope message bytes integer optional",
    ] {
        core.line(line);
    }
    core.message(Kind::Reply, "error", &schema::<Fault>())
        .expect("described");
    check(directory(), "core", CORE_VERSION, &core, write())
        .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_interface_s_family_is_described() {
    let mut family = Description::new();
    family
        .tagged(
            Kind::Request,
            &schema::<interface::Request>(),
            "method",
            Some("params"),
            |_| true,
        )
        .expect("described");
    family
        .tagged(
            Kind::Reply,
            &schema::<interface::Reply>(),
            "reply",
            None,
            |_| true,
        )
        .expect("described");
    check(
        directory(),
        interface::FAMILY,
        interface::VERSION,
        &family,
        write(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
}
