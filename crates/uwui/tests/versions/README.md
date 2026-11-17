# Fixtures of each version of uwui's documents

One document of each version of each format: a theme, a style sheet, a table of shortcuts
([ADR-0286](../../../../docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md)).
`src/versions_tests.rs` reads each one, with its node `version` and without it, and refuses it
once its version is raised past the reader's. `1.expected` records what version 1's fixtures
read as: each rule's selector and declarations, each token's value and each shortcut.

A fixture and what it reads as are never changed once a later version of its format exists: the
reader of that version must read the fixture, migrated, to the same record. A new version of a
format adds its own fixture, `<version>.<format>.kdl`, and its own record.
`SLIME_UPDATE_REFERENCE_IMAGES` writes `1.expected` again: a change of the reader must not
change it.
