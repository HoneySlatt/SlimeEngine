//! The agent, as its MCP client names itself, which the library names to a program.

use rmcp::RoleServer;
use rmcp::service::RequestContext;
use uwui_agent::Named;
use uwui_agent::page::cut;

/// The most bytes of JSON of the agent's name that the library names to a program, quotes
/// included: a program reads a hello of 4 KiB at most (`uwui_agent::Limits::HELLO`), and a
/// client's name is what it says it is.
pub const MAX_NAME: usize = 1024;

/// The most bytes of JSON of the agent's version, as [`MAX_NAME`] bounds its name.
pub const MAX_VERSION: usize = 256;

/// The agent that sent a request, as its MCP client names itself in MCP's handshake or in the
/// request itself, each part cut on a character's boundary to [`MAX_NAME`] and [`MAX_VERSION`]
/// bytes of JSON; unnamed when it gives no name.
pub fn agent_of(context: &RequestContext<RoleServer>) -> Named {
    context.client_info().map_or_else(
        || Named::new("an unnamed agent", ""),
        |client| named(client.name, client.version),
    )
}

/// `name` and `version`, cut to [`MAX_NAME`] and [`MAX_VERSION`] bytes of JSON.
fn named(mut name: String, mut version: String) -> Named {
    cut(&mut name, MAX_NAME);
    cut(&mut version, MAX_VERSION);
    Named::new(name, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwui_agent::{CORE_VERSION, Family, Hello, Limits, ToolSession};

    #[test]
    fn a_name_of_controls_is_cut_so_that_its_hello_fits_a_program_s_limit() {
        // Each control writes six bytes of JSON: 10,000 of them, some 60 KB, in the name and the
        // version alike.
        let controls: String = std::iter::repeat_n('\u{1}', 10_000).collect();
        let agent = named(controls.clone(), controls);
        assert!(uwui_agent::json_size(&agent.name) <= MAX_NAME);
        assert!(uwui_agent::json_size(&agent.version) <= MAX_VERSION);
        assert!(agent.name.starts_with('\u{1}'), "cut, not emptied");
        // The hello of the engine's MCP server, its token and its four families, with that agent.
        let hello = ToolSession::new()
            .hello(&Hello {
                core: CORE_VERSION,
                agent,
                client: Named::new("slime-mcp", "0.6.0"),
                secret: "0".repeat(32),
                families: ["play", "world", "editing", "interface"]
                    .into_iter()
                    .map(|family| Family::new(family, 9))
                    .collect(),
                limits: Limits {
                    line: 16 << 20,
                    bytes: 1 << 30,
                },
            })
            .expect("written");
        assert!(
            hello.len() <= Limits::HELLO.line,
            "a hello of {} bytes",
            hello.len()
        );
    }

    #[test]
    fn a_short_name_is_kept_whole() {
        let agent = named("claude-code".to_owned(), "2.1.304".to_owned());
        assert_eq!(agent, Named::new("claude-code", "2.1.304"));
    }
}
