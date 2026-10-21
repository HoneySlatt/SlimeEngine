//! An agent's session, played by this test: it speaks the Model Context Protocol to the engine's MCP
//! server on its standard input and output, starts *Hunna Wood* through it, pauses and steps it,
//! reads the hero, changes a value, walks the hero with a scripted key, captures the game's image,
//! reads its log and stops it (`docs/adr/0122-the-development-channel.md`).
//!
//! It needs a Wayland compositor, the game built (`cargo build -p hunnawood`) beside the server and
//! its zone cooked in `target/cooked/hunnawood`: `scripts/check.sh` runs it in a headless Weston
//! with lavapipe, as it runs the windowed lifecycle test (ADR-0024). It is not in the default run of
//! `cargo test` (`test = false` in Cargo.toml), and has no harness, so that its failures print once.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};

use base64::Engine as _;
use serde_json::{Value, json};

/// The server's ends of the conversation.
struct Session {
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Session {
    /// Sends `method` with `params`, and returns the result of the server's answer.
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.input, "{request}").expect("the server reads");
        self.input.flush().expect("the server reads");
        loop {
            let mut line = String::new();
            let read = self
                .output
                .read_line(&mut line)
                .expect("the server answers");
            assert!(read > 0, "the server ended before answering {method}");
            let answer: Value = serde_json::from_str(&line).expect("JSON");
            if answer["id"] == json!(id) {
                assert!(answer["error"].is_null(), "{method}: {answer}");
                return answer["result"].clone();
            }
        }
    }

    /// Calls the tool `name` with `arguments`, and returns the texts of its result and the result;
    /// it panics when the tool fails.
    fn tool(&mut self, name: &str, arguments: Value) -> (Vec<String>, Value) {
        let result = self.call(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let texts = result["content"]
            .as_array()
            .expect("content")
            .iter()
            .filter_map(|block| block["text"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        assert_ne!(result["isError"], json!(true), "{name}: {texts:?}");
        (texts, result)
    }
}

fn main() {
    let server = PathBuf::from(env!("CARGO_BIN_EXE_slime-mcp"));
    let target = server.parent().expect("the target directory");
    let game = target.join(format!("hunnawood{}", std::env::consts::EXE_SUFFIX));
    assert!(
        game.is_file(),
        "{} is not built: `cargo build -p hunnawood` first",
        game.display()
    );
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cooked = workspace.join("target/cooked/hunnawood");
    // In the build's own directory of temporary files, never the player's save; a run before this one
    // may have left it, with its lock.
    let save = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-drive.toml");
    let lock = save.with_extension("toml.lock");
    for file in [&save, &lock] {
        let _ = std::fs::remove_file(file);
    }

    let mut child = Command::new(&server)
        .current_dir(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the server starts");
    let mut session = Session {
        input: child.stdin.take().expect("piped"),
        output: BufReader::new(child.stdout.take().expect("piped")),
        next: 0,
    };
    let initialized = session.call(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "drive", "version": "1" },
        }),
    );
    assert!(
        initialized["capabilities"]["tools"].is_object(),
        "{initialized}"
    );
    assert_eq!(initialized["serverInfo"]["name"], json!("slime-mcp"));
    writeln!(
        session.input,
        "{}",
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
    )
    .expect("the server reads");
    let tools = session.call("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    for name in [
        "start_game",
        "pause",
        "step",
        "inspect_entity",
        "set_property",
        "capture",
        "save_game",
        "load_game",
    ] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }

    let command = json!([
        game.to_str().expect("a path of text"),
        "--new",
        "--save",
        save.to_str().expect("a path of text"),
        "--assets",
        cooked.to_str().expect("a path of text"),
    ]);
    let (said, _) = session.tool("start_game", json!({ "command": command }));
    assert!(said[0].contains("hunnawood runs"), "{said:?}");
    session.tool("pause", json!({}));
    let (hero, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert!(
        hero[0].contains("[Transform]") && hero[0].contains("[hunnawood::Look]"),
        "{hero:?}"
    );
    session.tool(
        "set_property",
        json!({ "entity": "hero", "component": "hunnawood::Look", "path": "yaw", "value": "1.0" }),
    );
    let (hero, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert!(hero[0].contains("yaw = 1.0"), "{hero:?}");
    // The game holds its time while its zone loads; steps asked for while paused run all the same:
    // W held for 30 steps walks the hero.
    let place = |hero: &[String]| {
        let transform = hero[0].split("[Transform]").nth(1).expect("a transform");
        transform
            .lines()
            .find(|line| line.starts_with("translation"))
            .map(str::to_owned)
    };
    let before = place(&hero);
    session.tool("press_key", json!({ "key": "KeyW" }));
    session.tool("step", json!({ "steps": 30 }));
    session.tool("release_key", json!({ "key": "KeyW" }));
    let (walked, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert_ne!(place(&walked), before, "the hero walked: {walked:?}");
    // Saved there, to a file of the test's own, walked on, then loaded: the hero is back where it
    // was saved.
    let saved = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-drive-saved.toml");
    let _ = std::fs::remove_file(&saved);
    let file = json!({ "file": saved.to_str().expect("a path of text") });
    session.tool("save_game", file.clone());
    assert!(saved.is_file(), "saved");
    session.tool("press_key", json!({ "key": "KeyW" }));
    session.tool("step", json!({ "steps": 30 }));
    session.tool("release_key", json!({ "key": "KeyW" }));
    let (further, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert_ne!(place(&further), place(&walked), "{further:?}");
    session.tool("load_game", file);
    let (loaded, _) = session.tool("inspect_entity", json!({ "entity": "hero" }));
    assert_eq!(place(&loaded), place(&walked), "{loaded:?}");
    let (state, _) = session.tool("game_state", json!({}));
    assert!(state[0].contains("paused"), "{state:?}");
    let (_, image) = session.tool("capture", json!({}));
    let blocks = image["content"].as_array().expect("content");
    let png = blocks
        .iter()
        .find(|block| block["type"] == json!("image"))
        .expect("an image");
    assert_eq!(png["mimeType"], json!("image/png"));
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png["data"].as_str().expect("base64"))
        .expect("base64");
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .expect("a PNG image");
    let mut pixels = vec![0; decoder.output_buffer_size().expect("a size")];
    let frame = decoder.next_frame(&mut pixels).expect("its pixels");
    assert!(frame.width > 0 && frame.height > 0);
    // The zone drawn, not a cleared image: many colors.
    let colors: std::collections::HashSet<&[u8]> =
        pixels[..frame.buffer_size()].chunks(4).collect();
    assert!(colors.len() > 1000, "{} colors", colors.len());
    let (lines, _) = session.tool("read_logs", json!({ "after": 0 }));
    assert!(lines[0].contains("development channel"), "{lines:?}");
    let (stopped, _) = session.tool("stop_game", json!({}));
    assert!(stopped[0].contains("the game ended"), "{stopped:?}");

    drop(session);
    let status = child.wait().expect("the server ends with its input");
    assert!(status.success(), "{status}");
    for file in [&save, &lock, &saved] {
        let _ = std::fs::remove_file(file);
    }
    println!("test drive ... ok");
}
