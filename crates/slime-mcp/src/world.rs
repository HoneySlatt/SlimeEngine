//! The family `world` as tools: an agent reads the game's world, an entity at a time or a list by
//! pages, and changes it.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, schemars, tool, tool_router};
use serde::Deserialize;
use slime_channel::{EntitySummary, Fault, Reply, Request, WORLD, json_size};
use uwui_mcp::{Family, List, PAGE_BYTES, Page, gather, next_line, text};

use crate::Server;

/// What the family `world` tells an agent among the server's instructions.
const INSTRUCTIONS: &str = "`list_entities` and `inspect_entity` read the game's world, the \
     lists a page at a time, each page that more follow starting with its `next`, which `cursor` \
     takes; `set_property` changes it, as the editor's commands do. They reach the program that the \
     game's tools reach.";

/// The family `world`, for the server.
pub(crate) fn family() -> Family<Server> {
    Family::new(WORLD, INSTRUCTIONS, Server::world_tools())
}

/// What `list_entities` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ListEntities {
    /// Text that the entities' names or persistent identities hold, whatever its case; empty for
    /// every entity.
    filter: String,
    /// Where the page starts: the `next` that the page before gave, with the same `filter`; left
    /// out for the list's start.
    cursor: Option<String>,
    /// The most entities wanted on the page; left out for as many as fit.
    limit: Option<u64>,
}

/// What `inspect_entity` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct InspectEntity {
    /// The entity's persistent identity, as `list_entities` gives it: `hero`, `zone/door`.
    entity: String,
}

/// What `set_property` takes.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SetProperty {
    /// The entity's persistent identity.
    entity: String,
    /// The component's type, as `inspect_entity` names it: `Transform`, `Light`.
    component: String,
    /// The part of the component, as a property path: `translation.x`, `intensity`; empty for the
    /// whole component.
    path: String,
    /// The new value, in the engine's notation, as `inspect_entity` writes values: `12.5`, `true`,
    /// `"text"`, `{ x = 1.0, y = 0.0, z = 2.0 }`.
    value: String,
}

#[tool_router(router = world_tools, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Lists the world's entities by persistent identity and name, a page at a \
                       time: a page that more follow starts with `next`, which `cursor` takes for \
                       the rest. Texts too long for a page are cut, and marked."
    )]
    async fn list_entities(
        &self,
        Parameters(ListEntities {
            filter,
            cursor,
            limit,
        }): Parameters<ListEntities>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let first = Entities(Request::Entities {
            filter,
            cursor,
            limit,
            bytes: Some(PAGE_BYTES),
        });
        Ok(match gather(self, &context, first, PAGE_BYTES).await {
            Ok(page) => described(page),
            Err(failed) => failed,
        })
    }

    #[tool(
        description = "An entity's parent and its components, each written as the engine's \
                       documents write it."
    )]
    async fn inspect_entity(
        &self,
        Parameters(InspectEntity { entity }): Parameters<InspectEntity>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        Ok(self.answer(&Request::Entity { entity }, &context).await)
    }

    #[tool(
        description = "Sets a part of an entity's component, as the editor's commands do: the value \
                       is checked against the component's type and its ranges."
    )]
    async fn set_property(
        &self,
        Parameters(SetProperty {
            entity,
            component,
            path,
            value,
        }): Parameters<SetProperty>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let request = Request::SetProperty {
            entity,
            component,
            path,
            value,
        };
        Ok(self.answer(&request, &context).await)
    }
}

/// A request of a page of the world's entities, `world/entities`, which `list_entities` reads page
/// after page.
#[derive(Clone, Debug, PartialEq)]
struct Entities(Request);

/// A page of entities, as `world/entities` answers it.
#[derive(Debug, PartialEq)]
struct EntityPage {
    entities: Vec<EntitySummary>,
    next: Option<String>,
}

impl Page for EntityPage {
    fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    fn extend(&mut self, more: Self) {
        self.entities.extend(more.entities);
        self.next = more.next;
    }

    fn size(&self) -> (u64, u64) {
        (self.entities.len() as u64, json_size(&self.entities) as u64)
    }
}

impl List for Entities {
    type Request = Request;
    type Reply = Reply;
    type Page = EntityPage;

    fn request(&self) -> &Request {
        &self.0
    }

    fn resumed(&self, cursor: String, count: u64, bytes: u64) -> Option<Self> {
        let mut request = self.0.clone();
        let Request::Entities {
            cursor: at,
            limit,
            bytes: most,
            ..
        } = &mut request
        else {
            return None;
        };
        if let Some(limit) = limit {
            *limit = limit.checked_sub(count).filter(|&left| left > 0)?;
        }
        *at = Some(cursor);
        *most = Some(bytes);
        Some(Self(request))
    }

    fn page(reply: Reply) -> Result<EntityPage, Fault> {
        match reply {
            Reply::Entities { entities, next } => Ok(EntityPage { entities, next }),
            other => Err(Fault::new(
                Fault::INVALID,
                format!("the game answered a list of entities with {other:?}"),
            )),
        }
    }
}

/// The result that lists `page`'s entities, each on its line, after its `next`.
fn described(page: EntityPage) -> CallToolResult {
    let mut lines: Vec<String> = page.next.as_deref().map(next_line).into_iter().collect();
    if page.entities.is_empty() {
        lines.push("no entity on this page".to_owned());
    }
    lines.extend(page.entities.into_iter().map(|entity| {
        let mut line = if entity.name.is_empty() {
            entity.identity
        } else {
            format!("{} ({})", entity.identity, entity.name)
        };
        if !entity.cut.is_empty() {
            line.push_str(&format!(" [cut: {}]", entity.cut.join(", ")));
        }
        line
    }));
    text(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(name: &str) -> EntitySummary {
        EntitySummary {
            identity: String::new(),
            name: name.to_owned(),
            cut: Vec::new(),
        }
    }

    fn page(names: &[&str], next: Option<&str>) -> EntityPage {
        EntityPage {
            entities: names.iter().map(|name| entity(name)).collect(),
            next: next.map(str::to_owned),
        }
    }

    /// The pages of a list gathered into one, with the last page's `next`.
    #[test]
    fn pages_are_gathered_with_the_last_next() {
        let mut gathered = page(&["a"], Some("1"));
        gathered.extend(page(&[], Some("2")));
        gathered.extend(page(&["b", "c"], None));
        assert_eq!(gathered, page(&["a", "b", "c"], None));
        assert_eq!(gathered.size().0, 3);
        assert_eq!(gathered.size().1, json_size(&gathered.entities) as u64);
    }

    /// The rest of a list is asked from the last cursor, within what is left of its `limit`, and
    /// not at all once the limit is reached.
    #[test]
    fn the_rest_is_asked_within_the_limit_left() {
        let first = Entities(Request::Entities {
            filter: "lamp".to_owned(),
            cursor: None,
            limit: Some(5),
            bytes: Some(PAGE_BYTES),
        });
        assert_eq!(
            first.resumed("c".to_owned(), 2, 100),
            Some(Entities(Request::Entities {
                filter: "lamp".to_owned(),
                cursor: Some("c".to_owned()),
                limit: Some(3),
                bytes: Some(100),
            }))
        );
        assert_eq!(
            first.resumed("c".to_owned(), 5, 100),
            None,
            "the limit reached"
        );
        assert_eq!(
            Entities(Request::State).resumed("c".to_owned(), 0, 100),
            None
        );
    }

    /// A reply that is no page of entities is a fault.
    #[test]
    fn a_reply_of_another_kind_is_no_page() {
        assert_eq!(
            Entities::page(Reply::Done).map_err(|fault| fault.code),
            Err(Fault::INVALID)
        );
    }
}
