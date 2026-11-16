#![doc = include_str!("../README.md")]
// A public enumeration may gain cases without breaking a program; one closed by nature says why
// it is not marked (ADR-0282).
#![warn(clippy::exhaustive_enums)]

mod agent;
mod carrier;
mod family;
mod interface;
mod pages;
mod result;

pub use agent::{MAX_NAME, MAX_VERSION, agent_of};
pub use carrier::{Answer, Carrier, Sent, ask, request};
pub use family::{Family, Owner, Taken, Tools};
pub use interface::{
    ActOnInterface, AnElement, ElementAt, Elements, FindElements, image, interface_family,
};
pub use pages::{LIST_TIME, List, PAGE_BYTES, Page, gather, page_bytes};
pub use result::{failure, fault, next_line, text};
/// The JSON that the core's messages hold, which a carrier sends and answers.
pub use uwui_agent::Value;
