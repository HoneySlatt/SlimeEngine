//! The check of rest that the tests of the editor and of the game run
//! (`docs/adr/0185-rest-checked-in-ci.md`), checked on a program that waits, which it finds at
//! rest, and on programs made to wake themselves, to spin or to end, which it does not. It reads
//! Linux's `/proc`, and runs there only.

#![cfg(target_os = "linux")]

#[path = "common/rest.rs"]
mod rest;

use std::process::{Child, Command};
use std::time::Duration;

/// Runs `script` in a shell, killed when the test ends, even by a panic.
struct Script(Child);

impl Script {
    fn start(script: &str) -> Self {
        Self(
            Command::new("sh")
                .args(["-c", script])
                .spawn()
                .expect("a shell starts"),
        )
    }
}

impl Drop for Script {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_program_that_waits_is_at_rest() {
    // `exec` makes the shell the waiting program itself, with no shell left to wait for it.
    let script = Script::start("exec sleep 60");
    rest::at_rest(script.0.id(), Duration::from_secs(5)).expect("at rest");
}

#[test]
fn a_program_that_wakes_itself_is_not_at_rest() {
    // The shell wakes each time its `sleep` ends, five times a second: never still for a second.
    let script = Script::start("while :; do sleep 0.2; done");
    let woke = rest::at_rest(script.0.id(), Duration::from_secs(2)).unwrap_err();
    assert!(woke.contains("switches"), "{woke}");
}

#[test]
fn a_program_that_wakes_itself_rarely_is_not_at_rest() {
    // Still for more than a second, then awake once every 2 seconds: the 10 seconds' watch finds it.
    let script = Script::start("while :; do sleep 2; done");
    let woke = rest::at_rest(script.0.id(), Duration::from_secs(5)).unwrap_err();
    assert!(woke.contains("alone"), "{woke}");
}

#[test]
fn a_program_that_spins_is_not_at_rest() {
    // Never waiting, the shell may never switch on a core left to it: its time on the CPU shows.
    let script = Script::start("while :; do :; done");
    let spun = rest::at_rest(script.0.id(), Duration::from_secs(2)).unwrap_err();
    assert!(!spun.contains(" 0 ticks"), "{spun}");
}

#[test]
fn the_process_that_start_game_names_is_read() {
    let pid = std::process::id();
    let said = format!("rest runs as process {pid}, its development channel at 127.0.0.1:1");
    let program = std::env::current_exe().expect("the test's program");
    assert_eq!(rest::pid(&said, &program), pid);
}

#[test]
fn a_program_that_ended_is_not_at_rest() {
    // Ended, and not waited for: a zombie, whose counts no longer change.
    let script = Script::start("exit 0");
    std::thread::sleep(Duration::from_millis(500));
    let ended = rest::at_rest(script.0.id(), Duration::from_secs(2)).unwrap_err();
    assert!(ended.contains("ended"), "{ended}");
}
