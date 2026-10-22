//! An agent's session that captures each view of a game through the engine's MCP server
//! (`docs/adr/0126-views-apart-from-the-renderer.md`): it starts the `world` example, which draws
//! the yard from a camera turning around it and a map from above in a corner, pauses it, reads the
//! names of its views, captures the whole image and each view, and stops it. The map, whose camera
//! does not move, is the part of the whole image that its rectangle covers. It then shows the map's
//! normals, a setting of the view changed through the server
//! (`docs/adr/0127-the-settings-of-a-view.md`): the ground, seen from above, faces up.
//!
//! It needs a Wayland compositor, the example built (`cargo build --example world`) and the
//! examples' assets cooked, in the directory that its first argument names, or
//! `target/cooked/examples`: `scripts/check.sh` runs it in a headless Weston with lavapipe, as it
//! runs the `drive` test. It is not in the default run of `cargo test` (`test = false` in
//! Cargo.toml), and has no harness, so that its failures print once.

#[path = "common/session.rs"]
mod session;

use std::path::Path;

use serde_json::json;

use session::{Session, image, server};

fn main() {
    let server = server();
    let target = server.parent().expect("the target directory");
    let example = target.join(format!("examples/world{}", std::env::consts::EXE_SUFFIX));
    assert!(
        example.is_file(),
        "{} is not built: `cargo build --example world` first",
        example.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = std::env::args()
        .nth(1)
        .map_or_else(|| workspace.join("target/cooked/examples"), Into::into);

    let (mut session, mut child, _) = Session::start(&workspace, "views");
    let command = json!([
        example.to_str().expect("a path of text"),
        "--assets",
        assets.to_str().expect("a path of text"),
    ]);
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("world runs"), "{said:?}");
    // Game time stands still: the ring stays where it is, and the map, whose camera does not move,
    // draws the same image at each frame once the yard's meshes are loaded, in the background,
    // which game time does not wait for: the map is compared with the whole image once two of its
    // captures, around the whole image's, are the same.
    session.tool("pause", json!({}));
    let mut tries = 0;
    let (width, height, whole, side, map) = loop {
        let (_, captured) = session.tool("capture", json!({ "view": "map" }));
        let before = image(&captured);
        let (_, captured) = session.tool("capture", json!({}));
        let (width, height, whole) = image(&captured);
        let (said, captured) = session.tool("capture", json!({ "view": "map" }));
        let (side, map_height, map) = image(&captured);
        assert_eq!(side, map_height, "the map is square: {said:?}");
        if before == (side, map_height, map.clone()) {
            break (width, height, whole, side, map);
        }
        tries += 1;
        assert!(tries < 20, "the map changes at every frame");
    };
    let (state, _) = session.tool("game_state", json!({}));
    assert!(state[0].contains("views `yard`, `map`"), "{state:?}");
    let (_, captured) = session.tool("capture", json!({ "view": "yard" }));
    let (yard_width, yard_height, _) = image(&captured);
    assert_eq!(
        (yard_width, yard_height),
        (width, height),
        "the yard fills the window"
    );
    assert!(
        side > 0 && side < width.min(height),
        "{side} in {width} × {height}"
    );
    // In the window's top right corner, a margin from its edges, as the example places it.
    let margin = (side as f32 * 0.05) as u32;
    let left = width - side - margin;
    let corner: Vec<u8> = (margin..margin + side)
        .flat_map(|row| {
            let start = ((row * width + left) * 4) as usize;
            whole[start..start + side as usize * 4].to_vec()
        })
        .collect();
    assert!(corner == map, "the map is the corner of the whole image");
    // The yard seen from above, not a cleared image: many colors.
    let colors: std::collections::HashSet<&[u8]> = map.chunks(4).collect();
    assert!(colors.len() > 100, "{} colors", colors.len());
    // The map's normals: the ground faces up, (0, 1, 0), which shows as (0.5, 1, 0.5), sRGB-encoded
    // in the window's image, within the dithering's step.
    let (said, _) = session.tool(
        "set_view_settings",
        json!({ "view": "map", "path": "debug", "value": "\"Normals\"" }),
    );
    assert_eq!(said, ["done"]);
    let (state, _) = session.tool("game_state", json!({}));
    assert!(
        state[0].contains("view `map`, its settings:") && state[0].contains("debug = \"Normals\""),
        "{state:?}"
    );
    let (_, captured) = session.tool("capture", json!({ "view": "map" }));
    let (_, _, normals) = image(&captured);
    let up = normals
        .chunks(4)
        .filter(|pixel| {
            [188_u8, 255, 188]
                .iter()
                .zip(*pixel)
                .all(|(&expected, &channel)| expected.abs_diff(channel) <= 2)
        })
        .count();
    assert!(
        up * 3 > normals.len() / 4,
        "{up} of {} pixels face up",
        normals.len() / 4
    );
    let (refused, result) = session.try_tool(
        "set_view_settings",
        json!({ "view": "map", "path": "render_scale", "value": "4.0" }),
    );
    assert_eq!(result["isError"], json!(true), "{refused:?}");
    let (refused, result) = session.try_tool("capture", json!({ "view": "minimap" }));
    assert_eq!(result["isError"], json!(true), "{refused:?}");
    assert!(
        refused[0].contains("no view `minimap`") && refused[0].contains("`yard`, `map`"),
        "{refused:?}"
    );
    let (stopped, _) = session.tool("stop_game", json!({}));
    assert!(stopped[0].contains("the game ended"), "{stopped:?}");

    drop(session);
    let status = child.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
    println!("test views ... ok");
}
