- The editor's journal names its format's version on its first line, `# journal 1`
  (`Editor::JOURNAL_VERSION`), and so does a copy on the clipboard; one without the line reads as
  version 1. A journal or a copy of a later version is refused with both versions named
  (`EditError::LaterJournal` for a journal), and the editor's program stops without writing the
  journal over (ADR-0297).
