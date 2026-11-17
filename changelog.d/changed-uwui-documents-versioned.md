- A style sheet whose rule is named `version` is refused: the name gives the sheet's version
  (ADR-0286).
- `DocumentError` holds `later`, the version of a document refused for being of a later version
  than uwui reads, kept even past `DocumentErrors::MOST`; a program never writes over such a file
  (ADR-0286).
- A dock's layout whose node `version` holds a later version and more is refused as later, and
  what the node of a known version holds beyond its number is left out with a warning, read
  leniently (ADR-0286).
