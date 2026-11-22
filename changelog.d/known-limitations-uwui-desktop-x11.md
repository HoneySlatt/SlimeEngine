- On X11, uwui's desktop host draws at one scale for the whole screen, the X resources'
  `Xft.dpi`; its touch is checked by hand alone (ADR-0313).
- On X11, where the server's focus follows the pointer, another program's grab of the pointer
  through XInput2 that ends over another window leaves uwui's desktop host told focused until the
  pointer crosses its window again (ADR-0313).
