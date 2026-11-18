- `World::load` and `World::about` read saves of every layout from 5 to the one the engine writes,
  each brought to the next through its upgrade; saves of layouts 1 to 4 are still refused with
  `DocumentError::Layout`, whose `read` is now the latest layout read. `Document::upgrade` brings a
  document of an earlier layout to its reader's before it is read (ADR-0288).
