# slime-files

How the engine, its tools and its games keep the files that the user cannot get back: written
whole or not at all, held by one program at a time, set aside rather than lost, in the user's own
directories
([ADR-0096](../../docs/adr/0096-the-files-the-engine-reads.md),
[ADR-0297](../../docs/adr/0297-one-crate-of-safe-files-and-the-editors-journal-versioned-built-by-s1.md)).

## Role

- `write`: a file written whole or not at all, through a file beside it forced to the disk and
  renamed, its directory synced where the system allows it; `WriteError` tells a file not written
  from one written whose directory could not be synced. A game's saves, the editor's levels,
  layouts, keys, speeds, recent levels and the start of each journal, and the cooker's metadata
  files and cooked data are written so. A journal's later entries are appended to it, as a log is,
  by the editor.
- `HeldFile`: a file that one program at a time holds, a game's save or an editor's journal,
  through the lock of a file beside it; `HoldError` says why one cannot be held.
- `set_aside`: a file that does not read, moved under a name of its own, never written over.
- `data_directory` and `state_directory`: the user's directories, as Windows and the XDG base
  directories give them.

It lives at the bottom of the graph, beside the vocabulary crates, so that every crate that writes
for the user reaches it without the GPU, the window or uwui: the cooker, `slime-editor` and every
game.

## Entry points

```rust
let directory = std::env::temp_dir().join(format!("slime-files-readme-{}", std::process::id()));
let file = directory.join("save.toml");
let held = slime_files::HeldFile::hold(file.clone())?;
slime_files::write(held.path(), b"hero = 3\n")?;
assert_eq!(std::fs::read(&file)?, b"hero = 3\n");
# drop(held);
# std::fs::remove_dir_all(&directory)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The rustdoc holds the contracts: what `write` promises on each system and what an error leaves,
what a lock holds, which name a file set aside takes.

## Dependencies

None: the standard library alone (ADR-0297). Its errors are written by hand, without `thiserror`.

## Must not

- Depend on any crate: a game links it whatever else it leaves out.
- Read or decide what a file holds: its callers read their formats, and choose what to set aside.
- Log: it returns every failure to its caller, which knows what the file was for.
