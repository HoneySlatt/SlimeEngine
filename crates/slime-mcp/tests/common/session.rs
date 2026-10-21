//! An agent's side of a conversation with the engine's MCP server, which the tests that drive a game
//! through it share.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use base64::Engine as _;
use serde_json::{Value, json};

/// The server's ends of the conversation.
pub struct Session {
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Session {
    /// Starts the server in `workspace`, and opens the conversation as an agent named `client`
    /// does; returns the session, the server's process and the result of `initialize`.
    pub fn start(workspace: &Path, client: &str) -> (Self, Child, Value) {
        let mut child = Command::new(server())
            .current_dir(workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let mut session = Self {
            input: child.stdin.take().expect("piped"),
            output: BufReader::new(child.stdout.take().expect("piped")),
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
        writeln!(
            session.input,
            "{}",
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
        )
        .expect("the server reads");
        (session, child, initialized)
    }

    /// Sends `method` with `params`, and returns the result of the server's answer.
    pub fn call(&mut self, method: &str, params: Value) -> Value {
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
