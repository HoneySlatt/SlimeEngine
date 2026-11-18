- uwui's desktop host ends a run on a panic, in the application's code or its own, without
  `App::stop` and without dropping the application, with the new status `EXIT_PANICKED`, 101
  (ADR-0298); a second termination signal, half a second or more after the first, ends it at
  once with `EXIT_TERMINATED`, 130 (ADR-0306).
