//! An agent's side of a conversation with the engine's MCP server, which the tests that drive a game
//! through it share.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

/// How long a session waits for any one message of the server: past the longest call, a game that
/// `start_game` gives 600 s to build and start, with room.
const PATIENCE: Duration = Duration::from_secs(900);

/// The server's ends of the conversation.
pub struct Session {
    input: ChildStdin,
    /// The server's lines, as a thread of the session's reads them, so that each read is bounded
    /// by [`PATIENCE`].
    output: mpsc::Receiver<String>,
    next: u64,
}

impl Session {
    /// Starts the server in `workspace`, and opens the conversation as an agent named `client`
    /// does; returns the session, the server's process and the result of `initialize`. The
    /// server, and the programs it starts, which inherit its environment, keep their data and
    /// their state in a directory of the session's own, emptied first, never in the user's: an
    /// editor's journals and its recent levels, a game's saves.
    pub fn start(workspace: &Path, client: &str) -> (Self, Child, Value) {
        Self::start_with(&server(), workspace, client)
    }

    /// Starts `server`, a build of the server, as [`Session::start`] starts this one's: what a
    /// measure times against another commit's build.
    pub fn start_with(server: &Path, workspace: &Path, client: &str) -> (Self, Child, Value) {
        let user = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("slime-mcp-user-{client}"));
        let _ = std::fs::remove_dir_all(&user);
        let (data, state) = (user.join("data"), user.join("state"));
        let mut child = Command::new(server)
            .current_dir(workspace)
            .env("XDG_DATA_HOME", &data)
            .env("XDG_STATE_HOME", &state)
            .env("APPDATA", &data)
            .env("LOCALAPPDATA", &state)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let (lines, output) = mpsc::channel();
        let read = BufReader::new(child.stdout.take().expect("piped"));
        std::thread::spawn(move || {
            for line in read.lines().map_while(Result::ok) {
                if lines.send(line).is_err() {
                    return;
                }
            }
        });
        let mut session = Self {
            input: child.stdin.take().expect("piped"),
            output,
            next: 0,
        };
        let initialized = session.call(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": client, "version": "1" },
            }),
        );
        session.notify("notifications/initialized", json!({}));
        (session, child, initialized)
    }

    /// Sends `method` with `params`, and returns the result of the server's answer.
    pub fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        loop {
            let answer = self.next_message(method);
            if answer["id"] == json!(id) {
                assert!(answer["error"].is_null(), "{method}: {answer}");
                return answer["result"].clone();
            }
        }
    }

    /// Sends `method` with `params` without waiting for its answer: its number, which the answer
    /// gives back.
    pub fn send(&mut self, method: &str, params: Value) -> u64 {
        self.next += 1;
        let id = self.next;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.input, "{request}").expect("the server reads");
        self.input.flush().expect("the server reads");
        id
    }

    /// The server's next message, an answer or a notification, read while `waiting` waits, within
    /// [`PATIENCE`].
    pub fn next_message(&mut self, waiting: &str) -> Value {
        let line = match self.output.recv_timeout(PATIENCE) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("the server answered nothing within {PATIENCE:?} while {waiting} waited")
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("the server ended before answering {waiting}")
            }
        };
        serde_json::from_str(&line).expect("JSON")
    }

    /// Sends the notification `method` with `params`, which the server answers not.
    pub fn notify(&mut self, method: &str, params: Value) {
        let notification = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        writeln!(self.input, "{notification}").expect("the server reads");
        self.input.flush().expect("the server reads");
    }

    /// Calls the tool `name` with `arguments`, and returns the texts of its result and the result;
    /// it panics when the tool fails.
    pub fn tool(&mut self, name: &str, arguments: Value) -> (Vec<String>, Value) {
        let (texts, result) = self.try_tool(name, arguments);
        assert_ne!(result["isError"], json!(true), "{name}: {texts:?}");
        (texts, result)
    }

    /// Calls the tool `name` with `arguments`, and returns the texts of its result and the result,
    /// whether the tool failed or not.
    pub fn try_tool(&mut self, name: &str, arguments: Value) -> (Vec<String>, Value) {
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
        (texts, result)
    }
}

/// The server's program, which cargo builds for the tests.
pub fn server() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_slime-mcp"))
}

/// The image of the result of a `capture`: its width, its height and its pixels, four bytes each.
pub fn image(result: &Value) -> (u32, u32, Vec<u8>) {
    let blocks = result["content"].as_array().expect("content");
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
    pixels.truncate(frame.buffer_size());
    (frame.width, frame.height, pixels)
}
