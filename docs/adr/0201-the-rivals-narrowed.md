# 0201. The rivals narrowed

- Status: Accepted; amends [0198](0198-screens-of-reference-and-the-rivals-measured.md) (the
  rivals of the screens "Slatty's library" and "Opium's songs") and
  [0200](0200-applications-measured-from-outside.md) (how the rivals run while measured)
- Date: 2026-11-06

## Context

[ADR-0198](0198-screens-of-reference-and-the-rivals-measured.md) named the rivals that set the
bar of each screen of reference: for Slatty's library, Steam and Heroic on the owner's accounts,
Nautilus and gThumb on the same 10,000 covers; for Opium's songs, Spotify and Cider on the owner's
accounts, Strawberry, Elisa and GNOME Music on the same 8,500 songs. M6d.0c measured them by
[ADR-0200](0200-applications-measured-from-outside.md)'s method, and found:

- Strawberry draws its interface with Qt's widgets on the CPU, and Elisa with Qt Quick; neither
  aims at the motion and the rates that uwui aims at. Strawberry's analyser draws 25 frames a
  second without end.
- GNOME Music has no table of all its songs: its lists stop at 100, and its main view is a grid
  of albums, another screen.
- Heroic has no account logged in on the owner's desktop, only on the Steam Deck; it is built on
  Electron, as Cider is.
- The owner's own Spotify playlists hold 13 songs at most: a table of 8,500 songs on the owner's
  account exists only in a public playlist.
- Spotify plays on the account's other devices, and Cider plays real music: a song played to
  measure them reaches the owner's speakers unless the player is muted.
- ADR-0200 runs a program without a session bus and behind a closed proxy. Elisa and Steam do not
  start without a bus, gThumb shows nothing without a user's folders, the closed rivals need
  their network, and Steam's client leaves the tree of processes that the method counts.

With every rival measured but Heroic, the three players in part, the owner decided on 2026-11-06
not to spend more time on applications whose aims are not uwui's.

## Decision

**The rivals kept.** Slatty's library: Steam on the owner's account, Nautilus and gThumb on the
same 10,000 covers. Opium's songs: Spotify and Cider on the owner's accounts. "Hello, World!"
and Zed's file are unchanged.

**Set aside:** Strawberry, Elisa, GNOME Music and Heroic. Their steps and their part of uwui's
`scripts/market.sh` are removed; the figures already taken of the three players are not kept.

**Spotify's table** is a public playlist of 9,998 songs, "Top 10,000 Songs of All Time", opened
on the owner's account at its start; the owner's own playlists are written beside it.

**The closed players play a song, muted in the player,** through the session's sound server; the
owner allowed it, Spotify's playback leaving the owner's other device.

**How the rivals run,** by uwui's `scripts/market.sh`, beside ADR-0200's method:

- each with a session bus of its own, whose configuration lets it start no service;
- the closed ones with their network, and copies of the owner's profiles, never the profiles
  themselves; Steam's copy without the games and with no library but its own;
- Steam under a process that adopts what its launcher leaves, so that all its processes are
  counted;
- the user's folders written in each profile, and the thumbnails of the covers made beforehand,
  as after a first visit, by the thumbnailer that Nautilus and gThumb call.

## Alternatives considered

- **All the rivals of ADR-0198:** some forty minutes more of measures, and the owner's time to log
  in to Heroic, for bars that uwui does not aim at.
- **Strawberry kept for its memory alone:** a figure that Opium's floor already sets.
- **GNOME Music on its grid of albums:** a screen other than Opium's.
- **Spotify on the owner's largest playlist:** 13 songs, no measure of a long table.
- **No song played in the closed players:** the cost of playing, where a player spends most of its
  time, unmeasured on the bar.

## Consequences

- The bar of each screen, in
  [uwui's quality bar](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#the-bar-of-each-screen),
  comes from fewer rivals, all of them drawn by the GPU.
- Opium's floor is better than the market's bar on its start, its memory, its rest, a song
  playing and its frames.
- Spotify's figures depend on a public playlist that its owner may change: its size is written
  beside them.
