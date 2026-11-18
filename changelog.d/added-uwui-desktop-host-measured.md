- uwui's desktop host writes a trace where `UWUI_TRACE` names a file: each frame and each input to
  the present of the image after it, in the Chrome Trace Event format. A blank window on its
  platform layer, `uwui-desktop`'s example `blank`, and one on winit, `uwui-blank-winit`; and
  `scripts/host-cost.py`, which measures the host's own cost above them: start, memory, binary,
  rest and input to present, in a headless sway at 240 Hz (ADR-0307).
