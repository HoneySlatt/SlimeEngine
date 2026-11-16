//! A server's tools, gathered by families, each with its paragraph of the server's instructions; a
//! tool's name taken twice is an error, not a tool replaced
//! (`docs/adr/0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md`).

use std::collections::HashMap;
use std::fmt;

use rmcp::handler::server::router::tool::ToolRouter;

/// A family of uwui's core for agents as tools: its name, `interface`, the tools that send its
/// requests, and the paragraph of the server's instructions that tells an agent of them.
pub struct Family<S> {
    name: &'static str,
    instructions: &'static str,
    tools: ToolRouter<S>,
}

impl<S> Family<S> {
    /// The family `name`, whose `tools` an agent learns of by `instructions`, a paragraph whose
    /// essential sentence comes first: a client may show the model a server's instructions cut.
    pub fn new(name: &'static str, instructions: &'static str, tools: ToolRouter<S>) -> Self {
        Self {
            name,
            instructions,
            tools,
        }
    }
}

/// Who gave a server a tool: the server itself, or a family by its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Owner {
    /// The server's own tools, which send no request of a family.
    Server,
    /// A family's tools.
    Family(&'static str),
}

impl fmt::Display for Owner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Server => formatter.write_str("the server's own tools"),
            Self::Family(name) => write!(formatter, "the family `{name}`"),
        }
    }
}

impl<S> fmt::Debug for Family<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Family")
            .field("name", &self.name)
            .field("tools", &self.tools)
            .finish_non_exhaustive()
    }
}

/// A tool's name that two owners give, which an agent could reach only one of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taken {
    /// The tool's name.
    pub tool: String,
    /// Who gave it first.
    pub first: Owner,
    /// The family that gave it again.
    pub second: &'static str,
}

impl fmt::Display for Taken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the tool `{}` of the family `{}` takes a name that {} took",
            self.tool, self.second, self.first
        )
    }
}

impl std::error::Error for Taken {}

/// Every tool of a server, its own and its families', each name given once, and the server's
/// instructions that tell an agent of them: its own paragraph first, then each family's, in the
/// order they were added. rmcp's routers, added up, replace a tool of a name already taken without a
/// word; this refuses it.
///
/// The server's type holds it behind an `Arc`, as each request clones the server.
pub struct Tools<S> {
    router: ToolRouter<S>,
    instructions: String,
    owners: HashMap<String, Owner>,
}

impl<S> fmt::Debug for Tools<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Tools")
            .field("router", &self.router)
            .field("owners", &self.owners)
            .finish_non_exhaustive()
    }
}

impl<S: Send + Sync + 'static> Tools<S> {
    /// The server's `own` tools, which send no request of a family, as starting a program, and
    /// `instructions`, its first paragraph: what the server is, and what crosses its families.
    pub fn new(instructions: &str, own: ToolRouter<S>) -> Self {
        let owners = own
            .map
            .keys()
            .map(|name| (name.to_string(), Owner::Server))
            .collect();
        Self {
            router: own,
            instructions: instructions.to_owned(),
            owners,
        }
    }

    /// Adds `family`'s tools, and its paragraph after the instructions.
    ///
    /// # Errors
    ///
    /// [`Taken`] when one of its tools takes a name already taken, disabled or not: then nothing
    /// is added.
    pub fn with(mut self, family: Family<S>) -> Result<Self, Taken> {
        // The first in the order of names, so that the error is the same at each start.
        let mut names: Vec<&str> = family.tools.map.keys().map(AsRef::as_ref).collect();
        names.sort_unstable();
        if let Some(&tool) = names.iter().find(|name| self.owners.contains_key(**name)) {
            return Err(Taken {
                tool: tool.to_owned(),
                first: self.owners[tool],
                second: family.name,
            });
        }
        self.owners.extend(
            names
                .iter()
                .map(|name| ((*name).to_owned(), Owner::Family(family.name))),
        );
        self.instructions.push_str("\n\n");
        self.instructions.push_str(family.instructions);
        self.router.merge(family.tools);
        Ok(self)
    }

    /// The router of every tool, which the server's handler calls and lists.
    pub fn router(&self) -> &ToolRouter<S> {
        &self.router
    }

    /// The server's instructions.
    pub fn instructions(&self) -> &str {
        &self.instructions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::handler::server::router::tool::ToolRoute;
    use rmcp::model::{CallToolResult, Tool};
    use std::sync::Arc;

    #[derive(Clone)]
    struct Server;

    /// A router of tools of the names `names`, which answer nothing.
    fn router(names: &[&'static str]) -> ToolRouter<Server> {
        names.iter().fold(ToolRouter::new(), |router, &name| {
            router.with_route(ToolRoute::new_dyn(
                Tool::new(name, "", Arc::default()),
                |_| Box::pin(async { Ok(CallToolResult::success(Vec::new()).into()) }),
            ))
        })
    }

    #[test]
    fn families_add_their_tools_and_their_paragraphs_in_order() {
        let tools = Tools::new("The server.", router(&["start"]))
            .with(Family::new("play", "Play.", router(&["pause", "step"])))
            .and_then(|tools| tools.with(Family::new("world", "World.", router(&["list"]))))
            .expect("no name taken twice");
        let mut names: Vec<String> = tools
            .router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["list", "pause", "start", "step"]);
        assert_eq!(tools.instructions(), "The server.\n\nPlay.\n\nWorld.");
    }

    #[test]
    fn a_name_taken_twice_is_refused_naming_both_owners() {
        let tools = Tools::new("The server.", router(&["start"]))
            .with(Family::new("play", "Play.", router(&["pause", "step"])))
            .expect("no name taken twice");
        let Err(taken) = tools.with(Family::new("world", "World.", router(&["list", "step"])))
        else {
            panic!("`step` taken twice");
        };
        assert_eq!(
            taken,
            Taken {
                tool: "step".to_owned(),
                first: Owner::Family("play"),
                second: "world",
            }
        );
        assert_eq!(
            taken.to_string(),
            "the tool `step` of the family `world` takes a name that the family `play` took"
        );
        let own = Tools::new("The server.", router(&["start"])).with(Family::new(
            "play",
            "Play.",
            router(&["start"]),
        ));
        assert_eq!(
            own.err().map(|taken| taken.to_string()).as_deref(),
            Some(
                "the tool `start` of the family `play` takes a name that the server's own tools took"
            )
        );
    }

    #[test]
    fn a_disabled_tool_s_name_is_taken_too() {
        let mut own = router(&["start"]);
        assert!(own.disable_route("start"));
        let added =
            Tools::new("The server.", own).with(Family::new("play", "Play.", router(&["start"])));
        assert!(added.is_err(), "a name hidden is still taken");
    }
}
