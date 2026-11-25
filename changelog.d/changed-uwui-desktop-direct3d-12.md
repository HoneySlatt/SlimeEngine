- uwui's desktop host opens Direct3D 12 by default on Windows, as the engine does; `WGPU_BACKEND`
  still names another. Its shaders compile with the `dxcompiler.dll` that lies beside the program,
  else with FXC, which the log warns of: a program on the host ships DXC beside it (ADR-0066,
  ADR-0317).
