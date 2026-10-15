# 0070. The sample game's source assets: free, listed, mirrored

- Status: Accepted
- Date: 2026-10-15

## Context

*Hunna Wood*'s first zone ([ADR-0067](0067-hunna-wood-replaces-nightwatch.md)) needs models and
textures: rocks, cliffs, ferns, logs, ground and stone textures, a door, lanterns, a campfire, a
hero.
The repository holds none, and the project has no artist to make them. The owner chose free assets
to start, to change later if the game needs it.

A survey on 2026-10-15 found:

- Poly Haven and ambientCG publish models and textures under CC0, which waives every right and asks
  nothing back. Poly Haven has no tree light enough for a game (its fir is 7.9 million triangles),
  no pieces of ruins, no torches and no people, and its glTF downloads leave out the alpha masks of
  plants and lanterns. Blender Studio's *Human Base Meshes* gives CC0 bodies, without textures; no
  realistic character ready for a game is CC0.
- Other free assets carry other licences, often on the same site: CC-BY, which asks for credit,
  most realistic trees among them, behind an account; CC-BY-SA; terms of their own.
- The zone's list comes to about 170 MB of downloads with textures 1024 texels wide, 450 MB at
  2048, and about 125 MB kept at 1024 once trimmed.
- In git, these files would stay in the history for good; GitHub refuses files over 100 MiB and
  advises repositories under 1 GB. Git LFS gives 10 GiB of storage and 10 GiB of downloads a month
  for free, which CI's checkouts count against; past them, LFS stops serving the files and CI
  fails.
- Fetched from where they are published, the files can change under their hash: ambientCG
  repackaged its archives in October 2025, and Poly Haven's files change when it exports them
  again.
- A GitHub release takes files up to 2 GiB each, a thousand per release, with no limit on
  downloads, and can be made immutable.

## Decision

- **CC0 only, for now.** Every source asset of the game is CC0, and the checks refuse an asset
  listed under another licence. What the free libraries lack is made in Blender (below).
- **A list in git.** One entry per source asset: where it comes from (site, page, author), its
  licence, the date it was taken, and for each of its files: the address it was published at, the
  address of its copy on the mirror, its SHA-256 and its size. The notice that ships with the game
  credits each source from it.
- **A mirror of the files.** The files themselves are attached to immutable releases of a GitHub
  repository of their own, which holds nothing else; a change to the list makes a new release.
  `scripts/fetch-assets.sh` downloads each file from the mirror, from where it was published if the
  mirror lacks it, checks its SHA-256, and puts it in the game's source directory,
  `games/hunnawood/assets/`, where git tracks the metadata files and ignores the rest. CI keeps
  what the script fetched in its cache, keyed by the list.
- **The zone is built by a script.** A Python script, run by Blender at a version it names, and a
  readable file of placements, both in git, build the zone from the fetched files: the script puts
  back the alpha masks, makes what the libraries lack (trees whose foliage is *leaf cards*, ruins,
  the hero's clothes, lanterns), places the colliders and markers as custom properties, sets the
  lights of day and night, and exports glTF. The export is a source of the pipeline like the
  others, listed and mirrored, so that CI and Windows never need Blender, and so that everyone cooks
  the same file: Blender's output is not reproducible byte for byte. `.blend` files are never
  committed.
- **The engine's own test assets stay in git**: the sample scene, `examples/assets/scene.glb`, is
  small and made by a script, `examples/assets/make_scene.py`, as before.

The owner chose this on 2026-10-15.

## Alternatives considered

- **Files in git**: the history grows by hundreds of megabytes for good, and every clone downloads
  them.
- **Git LFS**: past the free quota, which CI would reach, LFS stops serving the files.
- **Only the addresses where the files are published, with their hashes**: nothing to host, but
  the files change there and the hashes fail.
- **The mirror as releases of SlimeEngine itself**: no second repository, but the engine's releases
  would mix with snapshots of assets.
- **CC-BY trees and characters**: better looking, but each needs an account to download, credit
  for each author, and more licences for the checks to accept. The owner chose trees made in
  Blender instead.
- **Everything made in Blender**: one licence, no download, but no textures of the quality of
  photographed ones.

## Consequences

- A clone of the repository does not hold the game's assets: `scripts/fetch-assets.sh` fetches them
  first, a few hundred megabytes, after which work goes on offline.
- An asset changed where it was published changes nothing until the list changes.
- Changing the zone means running the build script in Blender, then mirroring its export.
- How the zone looks depends on the time spent making it in Blender, which `docs/PLAN.md` bounds
  with budgets.
