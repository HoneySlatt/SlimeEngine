# Fixtures of each version of uwui's documents

One document of each version of each format: a theme, a style sheet, a table of shortcuts
([ADR-0286](../../../../docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md)).
`src/versions_tests.rs` reads each one, with its node `version` and without it, and refuses it
once its version is raised past the reader's.

A fixture is never changed once its version is out: the reader of a later version must read it,
migrated. A new version of a format adds its own fixture, `<version>.<format>.kdl`.
