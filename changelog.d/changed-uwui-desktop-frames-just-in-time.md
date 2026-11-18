- uwui's desktop host draws its frames just in time on Wayland: a frame starts as late as its work
  allows before the compositor composes, so that input in motion reaches the next image; it reads
  when each image showed through `wp_presentation`, gives the interface that time predicted
  (`uwui::time::FrameSource::Predicted`) and reads the display's period rather than measuring it.
  Its trace tells when each image showed and the frames that missed their refresh, and
  `scripts/host-cost.py` measures input to shown beside input to present, and inputs in motion
  (ADR-0309).
