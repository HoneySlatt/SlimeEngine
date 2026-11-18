- `slime-files`, a crate on the standard library alone, holds how the engine, its tools and games
  keep the files that the user cannot get back: `write` writes a file whole or not at all and syncs
  its directory where the system allows it, its `WriteError` telling a file not written from one
  written but not synced, `HeldFile` holds a file for one program, its errors a
  `HoldError` to match, `set_aside` moves aside a file that does not read, and `data_directory` and
  `state_directory` give the user's directories (ADR-0297).
