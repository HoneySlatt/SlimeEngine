- uwui's desktop host takes a touch screen's fingers on Wayland, each a pointer of its own, the
  first followed by the interface; it stands on a platform layer of its own, `uwui-windowing`,
  which knows neither the interface nor the GPU (ADR-0279, ADR-0302).
