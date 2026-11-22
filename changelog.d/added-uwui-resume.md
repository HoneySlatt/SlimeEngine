- uwui tells a program that the system resumed, with the time suspended:
  `ActionKind::Resumed` in its queue of actions, which a host gives through `Ui::resumed`. uwui's
  desktop host gives it on Linux, on Wayland and on X11, as soon as the system resumes, from the
  kernel's own notice, its window at rest too, where the kernel measures the sleep (ADR-0315).
