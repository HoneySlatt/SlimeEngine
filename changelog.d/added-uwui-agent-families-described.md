- The messages of uwui's core for agents and of each family, `interface`, `play`, `world` and
  `editing`, are described in a file kept with their code, one line for each message, value and
  field, and held to their versions by a test, as `uwui_agent::description` says: a change that an
  end of before would misread fails until the family's version is raised (ADR-0308).
