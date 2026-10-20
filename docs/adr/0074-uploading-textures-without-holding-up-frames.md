# 0074. Uploading textures without holding up frames

- Status: Accepted; amends [0073](0073-loading-scenes-in-the-background.md) (what a load costs the
  frames); amended by [0107](0107-the-execution-model.md) (one owner of the copies to the GPU)
- Date: 2026-10-15

## Context

[ADR-0072](0072-stage-4b-live-assets.md) sets the bar of Stage 4b's loading: while *Hunna Wood*'s
zone loads, no interval between frames above 33 ms on the owner's RX 6900 XT, in a release build at
1280 × 720. [ADR-0073](0073-loading-scenes-in-the-background.md) loads scenes on the renderer's
threads, four on that machine, and expects the thread that submits frames to wait while those
threads upload textures, since wgpu's `write_texture` copies the texels into wgpu's staging memory
while holding a lock that every submission takes.

M4b.2 made the game draw while its zone loads, and measured the interval between the starts of two
frames, from the first frame to the one after the zone is first drawn, with the files in the
system's cache. Three things held up frames:

- **The queue's lock.** With four threads uploading through `write_texture`, and the hero drawn
  from the first frame (below), 35 runs in 40 had an interval above 33 ms, up to 59 ms: the
  renderer's submission waited behind the copies.
- **The first shadows.** The game first drew its sky alone, then its hero once loaded, then its
  zone. The first frame that draws something casting a shadow makes the renderer create its
  shadow atlas, 64 MiB, and its frame again, with targets of a new shape. That frame came in the
  middle of the zone's load, and the GPU, busy with the zone's copies, presented it late. With the
  texels written outside the lock (below), runs with an interval of 34 to 52 ms there were 5 in 80
  with one thread, 7 in 30 with two, and 8 in 30 with four.
- **The night sky.** Its moon is a light of the zone, so the game computed it once the zone was
  loaded, on the main thread: about 23 ms in one frame.

Once the zone is drawn, nothing loads: in 1 800 frames, no interval passed 25 ms.

## Decision

- **`slime-gfx` uploads a texture's levels through buffers of its own**, which the CPU writes while
  they are mapped, then copies them into the texture with commands that it submits at once. The
  texels are written outside any lock of wgpu's; the submission takes the queue's lock for as long
  as it takes to hand the commands over. A buffer holds at most the device's largest, and an image
  too large for one is cut into bands of rows; each copy starts at a multiple of 512 bytes, the
  placement that Direct3D 12 asks of a copy's data on drivers without unrestricted pitches, where
  wgpu would otherwise go through a buffer of its own. Every texture is created this way, on any
  thread.
- **`slime-render` gives the lights of a cooked scene without loading it** (`scene_lights`), as
  `LoadedScene::lights` holds them, so that a game knows its sun and its moon before its scene is
  loaded.
- ***Hunna Wood* loads its hero before its first frame**, while it computes both skies, the moon's
  from its zone's lights read at startup, and loads its zone in the background: the frame keeps its
  shape, shadows included, from the first frame on, and the zone's arrival only adds instances.
- The renderer keeps four threads at most (ADR-0072).

Measured with these, as before: 40 runs by day and 20 at night, the longest interval of each run
between 8 and 22 ms. The zone was drawn from the first frame in 12 runs of the 60, and after one to
four frames in the others: the hero's load and the skies leave it little left to load on that
machine.

## Alternatives considered

- **Fewer loading threads, and the queue's writes kept**: fewer threads make fewer copies wait for
  the lock, not each copy shorter, and they load the zone slower. Measured with the texels already
  written outside the lock, one or two threads kept intervals below 22 ms where no shadow first
  appeared, but the first shadows still pushed 5 runs in 80, or 7 in 30, past 33 ms.
- **The fallback of ADR-0072, uploads from the main thread within a budget of bytes per frame**:
  more code in the game's loop and a slower load, for a problem that writing the texels outside
  the lock removes.
- **Creating the shadow atlases with the renderer**: the first shadows would cost no frame, but
  every program would hold 64 MiB more, shadows or not (ADR-0064 creates them when first drawn).
- **The hero in the background too, behind the sky alone**: the window would show its first frame
  about 70 ms sooner, but the frame that first draws the hero would come during the zone's load.

## Consequences

- A texture costs a submission when it is created, a small cost once per texture, and its levels
  go through buffers padded to the rows' alignment of copies, 256 bytes.
- A thread that loads no longer holds up the thread that submits frames by more than the time to
  hand over a submission; ADR-0073's expectation of a wait as long as each copy no longer holds.
- A game that changes the shape of its frame while something loads, by drawing the first shadows
  or by enabling a pass, pays that change during the load; *Hunna Wood* avoids it by drawing its
  hero from the first frame.
- The window of *Hunna Wood* shows its first frame after the renderer, the hero and the two skies:
  145 to 180 ms on the RX 6900 XT. Before Stage 4b it showed nothing while the renderer, the zone,
  the hero and the skies were made one after the other: about 400 to 500 ms, an estimate from the
  zone's and the hero's loads that ADR-0073 measured, 346 to 394 ms together in each run, and
  the renderer's and the skies' time.
