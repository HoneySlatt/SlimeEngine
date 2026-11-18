- A crash of the editor after its level's file is written but before its journal says so no longer
  stops the next start: `Editor::recover` takes a file that holds what the journal's steps give as
  written, and adds the journal's lost mark (ADR-0303).
