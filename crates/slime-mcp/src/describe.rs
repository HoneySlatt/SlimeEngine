//! What a reply of the engine's families tells an agent.

use rmcp::model::CallToolResult;
use slime_channel::{Fault, Reply};
use uwui_mcp::{failure, fault, image, text};

/// The result that tells `reply`, with `payload`, an image's pixels, after it, which are encoded
/// off the server's one thread.
pub(crate) async fn describe(reply: Reply, payload: Vec<u8>) -> CallToolResult {
    match reply {
        Reply::Done => text("done"),
        Reply::Failed { code, message } => fault(Fault::new(code, message)),
        Reply::State { state } => text(format!(
            "frame {}, game time {}: {} fixed steps run, {:.3} s of game time, at a speed of {}; \
             {:.3} s of real time; frames of {:.2} ms on average, {:.2} ms at the longest, over the \
             last ones; {} entities; {}; {}{}",
            state.frame,
            match (state.paused, state.held) {
                (true, true) => "paused, and held by the game, as while it loads",
                (true, false) => "paused",
                (false, true) => "held by the game, as while it loads",
                (false, false) => "running",
            },
            state.steps,
            state.game_seconds,
            state.speed,
            state.real_seconds,
            state.frame_ms,
            state.longest_frame_ms,
            state.entities,
            if state.views.is_empty() {
                "no view named".to_owned()
            } else {
                let names: Vec<&str> = state.views.iter().map(|view| view.name.as_str()).collect();
                format!("views `{}`", names.join("`, `"))
            },
            if state.reports.is_empty() {
                "no report".to_owned()
            } else {
                format!("reports `{}`", state.reports.join("`, `"))
            },
            state
                .views
                .iter()
                .map(|view| if view.settings.is_empty() {
                    format!("\n\nview `{}`: its settings are not given", view.name)
                } else {
                    format!(
                        "\n\nview `{}`, its settings:\n{}",
                        view.name,
                        view.settings.trim_end()
                    )
                })
                .collect::<String>(),
        )),
        Reply::Entity {
            entity,
            parent,
            components,
        } => {
            let mut said = entity.identity;
            if !entity.name.is_empty() {
                said.push_str(&format!(", named {}", entity.name));
            }
            if !parent.is_empty() {
                said.push_str(&format!(", child of {parent}"));
            }
            for component in components {
                said.push_str(&format!("\n\n[{}]\n{}", component.name, component.value));
            }
            text(said)
        }
        Reply::Report { text: report } => text(report),
        Reply::Picked { entity, found } => text(if !found {
            "nothing is picked there".to_owned()
        } else if entity.identity.is_empty() {
            format!(
                "an entity without a persistent identity, named `{}`",
                entity.name
            )
        } else if entity.name.is_empty() {
            entity.identity
        } else {
            format!("{} ({})", entity.identity, entity.name)
        }),
        Reply::Image { width, height } => image(width, height, payload).await,
        Reply::History { history } => {
            let list = |labels: &[String]| {
                if labels.is_empty() {
                    "none".to_owned()
                } else {
                    format!("`{}`", labels.join("`, `"))
                }
            };
            text(format!(
                "level {}; steps done, the oldest first: {}; steps undone, the next to redo first: \
                 {}; selected: {}; steps that the level's file does not hold: {}; entries of a \
                 journal recovered at the start: {}; journal {}",
                history.level,
                list(&history.done),
                list(&history.undone),
                list(&history.selection),
                history.unwritten,
                history.recovered,
                history.journal
            ))
        }
        Reply::Operations { operations } => text(
            operations
                .iter()
                .map(|operation| {
                    let parameters = if operation.parameters.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", operation.parameters.join(", "))
                    };
                    format!(
                        "{}{parameters}: {}; changes the {}",
                        operation.name, operation.label, operation.changes
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Reply::Written { left_out } => text(if left_out.is_empty() {
            "the level is written, whole".to_owned()
        } else {
            format!(
                "the level is written; its file could not hold:\n{}",
                left_out.join("\n")
            )
        }),
        Reply::Played { address, .. } => text(format!(
            "the editor plays a game, which listens at {address}"
        )),
        // Lists are read by their pages, the interface by its family's own replies, each by its
        // tool: none of them reaches here.
        reply @ (Reply::Entities { .. }
        | Reply::Journal { .. }
        | Reply::Logs { .. }
        | Reply::Interface { .. }) => failure(format!(
            "the program answered {reply:?}, which this tool does not read"
        )),
    }
}
