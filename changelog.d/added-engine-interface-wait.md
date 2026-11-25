- An agent waits for a change of the editor's or of a game's interface: the engine holds
  `interface/wait` on its window's target, beside the journal's waits and within the same count
  of 8 a connection, and answers each in the frame that shows what it waits for, after the
  presentation's `extract` or, for an interface framed as it draws, after `render`; a run at rest
  with a wait in flight runs no frame for it. A presentation frames its interface in one of the
  two (`Presentation::ui`). A wait on another target is refused (ADR-0316).
