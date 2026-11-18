- A panic during a run, in the game's code or in the engine's that the run calls, no longer
  unwinds out of `slime_app::run`: the run ends at once, no method of the game's called after it,
  either `stop` included, as before, and `run` returns `RunError::Panicked` with the panic's
  message, rather than the process ending as a panic does. *Hunna Wood* then exits with a failure,
  its last save left as it was (ADR-0298).
