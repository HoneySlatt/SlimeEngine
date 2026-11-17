- The engine's MCP server answers an agent's other calls while a game starts, stops or is driven:
  its other tools say at once that the game starts or stops. `start_game` is given up by its call's
  cancel or by `stop_game`, which kills the program that it started. `stop_game` closes the game's
  standard input, which ends it as a termination signal would even while long requests hold every
  place in flight, and kills it if it has not ended within 120 s. Of two calls of `drive_played_game`, the later wins. A
  capture is written as PNG off the server's thread (ADR-0274).
