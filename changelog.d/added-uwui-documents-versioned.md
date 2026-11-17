- uwui's themes, style sheets and tables of shortcuts carry the version of their format, `version 1`
  their first node, as docks' layouts do: a document without it reads as version 1, and one of a
  later version is refused whole, its error naming both versions and keeping the later one
  (`DocumentError::later`); no rule of a style sheet is named `version` (ADR-0286).
