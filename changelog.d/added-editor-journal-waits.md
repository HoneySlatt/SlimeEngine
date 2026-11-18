- An agent waits for an editor's next steps: `editing/journal` takes `wait`, answered once the
  editor writes past its cursor, and its page at the journal's end gives `end`, a cursor there;
  the engine's MCP server's `editor_journal` takes `wait`, in seconds, at most 50, and gives `end`.
  The editor holds the request without running a frame, and answers it in the frame that writes
  the entries; a connection holds at most 8 such requests (`slime_channel::MAX_WAITS`), one more
  answered `BUSY`. A simulation answers it through `Simulation::wait` (ADR-0310).
