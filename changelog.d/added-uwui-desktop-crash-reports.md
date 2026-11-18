- uwui's desktop host writes a crash report when a panic ends a run, and a session's marker that
  the next start finds when a run did not end, in the application's crash directory under the
  user's state directory, with the hours of use and the crashes counted; nothing leaves the
  machine. `WindowSettings::version` names the application's version in its reports (ADR-0306).
