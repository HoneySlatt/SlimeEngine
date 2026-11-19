- The interface's family rises to 4, for `interface/wait`: a tool built before and a program built
  after, or the other way round, refuse the family between them (ADR-0314).
- `uwui::agents::Waits::wait` takes the numbers of the elements seen, `after`, none to read the
  interface as it stands; `Waited::Met` and `Met` give the elements that met the wait and were not
  seen apart from those seen; a wait, or a check, while its target holds a change not yet framed
  reads nothing until the check after that frame (ADR-0314).
