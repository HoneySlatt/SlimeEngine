# 0102. A damaged file gives an error, never a crash

- Status: Accepted; amends [0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)
  (a light that cannot be drawn is left out), [0069](0069-the-asset-pipeline.md) and
  [0071](0071-cooked-textures.md) (who reads KTX2 files, and the cooked files reused),
  [0076](0076-hot-reload-of-cooked-data.md) (the cooked files reused),
  [0086](0086-serialization.md) (the bound of a document) and
  [0091](0091-scene-documents-and-placements.md) (the bound of a resolved scene)
- Date: 2026-10-19

## Context

[ADR-0096](0096-the-files-the-engine-reads.md) says that no file the engine reads may make the
program panic, hang, exhaust its memory or lose the player's progress. The audit of 0.4
([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)) found readers that broke
that rule; the audit pass's second milestone, P2, fixes them. [`docs/PLAN.md`](../PLAN.md)
describes each finding by its number:

- **G04.** The rule of a valid light was copied in the cooker, the world and the renderer, and not
  applied at every entry: the renderer's reader of cooked scenes checked a light's position and
  direction only; the world checked each `Transform` alone, never their composition; the sky did not
  check its sun; nothing bounded a color times an intensity; and `Renderer::render` panicked on a
  light it could not draw, as ADR-0042 decided for values that came from a game's code.
- **G05.** The readers of cooked data trusted the cooker: a manifest's name was joined to the
  directory as it was, a cooked file read whole before its size was compared, a pipe read without
  end; the `ktx2` crate panicked on one byte of a texture's data format descriptor, and its levels
  were copied before they were checked; a texture of the wrong kind for its slot tripped an
  `assert!`; the asset cache stayed loading after a panic that a game caught.
- **G06.** The limits of documents and scenes did not bound memory: a save of 16 MiB of empty
  arrays, and a few small scene files that placed a large prefab many times, each took gigabytes.
- **G07.** Reading a document that gave many warnings took time quadratic in its size.
- **G19.** The cooker reused any file under the expected name, and renamed what it wrote without
  forcing it to the disk.
- **G21.** The glTF importer read a `.gltf` file's buffers wherever their paths pointed, and
  computed tangents that were not numbers from positions far apart, on which the cooker panicked.
- **G22.** JPEG images in CMYK were cooked with wrong colors.
- **G41.** A panic in a game's upgrade or check left a world half loaded.

## Decision

**One rule says what a light may be.** `slime_assets::SceneLight::check` accepts a light whose
color channels and intensity are finite and not negative, whose color times its intensity is
finite in 32-bit floats, as the renderer's shader reads it, whose range is positive and finite, and
whose cones, for a spot light, lie within 0 ≤ inner < outer ≤ π / 2. The cooker applies it to the
lights it imports, the reader of cooked scenes to the lights it reads, the world's `Light` component
when reflection reads one, and the renderer's `Light::check`, public, which adds a position finite,
a direction finite and not zero and, for a directional light, a shadow distance positive and finite.
`slime-assets` holds it because every crate that reads a light depends on it.

- `Renderer::render` leaves out a light that `Light::check` refuses, and logs a warning that names
  it by its index in the scene's lights when it is first left out, not at every frame; it draws the
  first `MAX_LIGHTS` of the others. ADR-0042 said that such a light panics, as a bug in the game's
  code: lights now come from the world's data too ([ADR-0084](0084-what-the-world-draws.md)), which
  a player edits and a disk damages.
- The frame builder of `slime-world-render` checks each light where its entity's transforms,
  composed, place it, each of them finite, and leaves out, with a warning that names the entity, a
  light placed where no number reaches; the sky's sun is chosen among the others.
- The scene shader bounds the light that one light gives a point, before the surface reflects it,
  to 10³⁰ (`LARGEST_LIGHT`), and the sky bounds its sun's disc to as much: a light far brighter than
  any, its strength finite, gives no infinity that the shader multiplies by 0. `create_environment`
  panics on a sun that `Light::check` refuses, a bug of the caller's, as for the sky's other values.

**Cooked data is checked before it is used.**

- A manifest names its cooked files by ASCII letters, digits, `-`, `_` and `.`, not starting with
  `.`: a name that would leave the directory refuses the manifest. A name that a system reserves,
  as `NUL` on Windows, is refused when the file is read.
- `CookedAssets` refuses a cooked file that is not a file, as a directory, a pipe or a device is
  not, or that is of another size than the manifest says, before it opens it, and reads it up to
  that size.
- `CookedTexture::from_ktx2` reads KTX2 files with a reader of the engine's own, for the layout that
  the cooker writes: it checks the header, the sides, at most `slime_assets::MAX_SIDE`, 8192, which
  the GPU takes with the engine's default limits, the count of levels and each level's place and
  size, before it copies any; a texture takes at most its file's size. The `ktx2` crate only writes.
- A cooked material that reads a texture of the wrong kind for its slot, data where it reads colors
  or the reverse, is refused with `RenderError::InvalidMaterial`. `Renderer::create_material` still
  panics on such a texture, a bug of the game's code.
- `AssetCache::poll` ends a batch whose load panicked, and does not ask for its meshes again, as for
  a batch that failed.

**Memory is bounded before it is taken.**

- A document holds at most `slime_serialize::MAX_VALUES` values, 2²⁰. A text's are counted before
  it is parsed, by one for each `=`, `,`, `[`, `{` and `.` that it holds: every value but the root
  follows one of them, a table that a dotted key makes included, so that the count bounds what the
  parser makes, while a float's point or a quoted key counts too. The binary encoding's are counted
  as they are read. A writer refuses a document that a reader would. ADR-0086 said that the densest
  document of 16 MiB took 1.8 GB to read: such a document is now refused at once, and the most that
  a document read takes is in the rustdoc of `MAX_VALUES`.
- A resolved scene takes at most `slime_world::MAX_SCENE_SIZE` bytes, 256 MiB, counted for its
  records and its reports of what was set aside, their texts and, for each value, the size of a
  `Value`; the copies that its placements make of their prefabs are counted before any is made, as
  `MAX_ENTITIES` counts their entities; and so are the scenes that a save spawns while it is
  loaded, together. ADR-0091 counted entities only, which a large prefab placed many times passed.
- A lenient reading keeps the first `MAX_WARNINGS` warnings, 100, then one that counts the rest, and
  places the first thousand errors and warnings: in a text each from the one placed before, in the
  binary encoding along their paths, its strings read once and each table or array walked through
  once, its entries kept.

**The cooker checks what it reuses, and writes whole.** A cooked file is reused only if its kind's
reader reads it, of the size that the record of its import says for what a glTF file gave, and a
scene file's encoding only byte for byte: a file deleted, emptied or cut short is cooked again.
ADR-0069, ADR-0071 and ADR-0076 said that what is in the output under its name is not cooked again.
Every file the cooker writes, the metadata files it gives included, goes through a file beside it,
forced to the disk, then renamed. The glTF importer reads a buffer only from a file inside the
directory of sources, up to the length that the file declares, and refuses normals or tangents
computed from positions so far apart that they are not finite. JPEG images are asked of the
decoder in RGB, which zune-jpeg 0.5.15 converts right from CMYK.

**A panic in a game's check or upgrade refuses its value.** Reflection calls them so that a panic
sets the value aside as their error would, and the world loads as it would have without it.

**The damaged-input tests share one helper.** Every crate that reads files gives its readers valid
files cut at every byte, each byte changed to 0, to 255 and with its lowest bit flipped, and 2 000
copies mutated at random from a fixed seed, through `crates/slime-assets/tests/common/damage.rs`,
which the tests of `slime-serialize`, `slime-world` and `slime-cook` include with `#[path]`, as
`slime-render`'s tests share their common files. It checks that no read panics, and that the reads
of one file take less than 20 s in a debug build. `scripts/check.sh` runs the `scene` example on a
copy of the cooked sample with one byte of a mesh changed, which ends with an error, not a panic.

## Alternatives considered

- **Each crate keeps its rule of a light**, as before: the copies had drifted already, and the
  reader of cooked scenes and the sky had none. **The renderer returns an error for a light it
  cannot draw**: a frame refused whole for one light, which ADR-0042 rejected for a 17th light.
  **The light's values clamped into their ranges**: a damaged light drawn as a plausible one,
  without a word.
- **The `ktx2` crate's reader kept, its panic caught**: a panic is a bug of the program under
  ADR-0008, and catching it would not bound the levels it copies. **The crate patched first**: the
  engine would wait for a release, and reads one layout only, its own; the defect is to report to
  the crate's authors, with the owner's agreement.
- **A counting allocator in the tests, to measure what a read allocates**: it needs `unsafe` code
  in crates that have none, which ADR-0005 makes an architecture decision, for a property that the
  checks before allocation give by construction. **A lexer of TOML to count values exactly**: more
  code, which must follow the parser's grammar; the rough count costs a float its point.
- **A limit on a resolved scene's values rather than its bytes**: a prefab of one long text placed
  many times would pass it.
- **`Renderer::create_material` returning an error for a texture of the wrong kind**: a change of
  its contract for a bug of the caller's; the loader of cooked scenes, which reads the files,
  checks.
- **Every cooked file reused checked by a checksum**: ADR-0096 rejected checksums as the rule; a
  reader must be safe on a file that matches one. **The size alone checked**: it finds what is cut
  short, not a header damaged. **Everything cooked again at each cook**: textures take seconds each.
- **A crate of test helpers shared by the crates' tests**: a crate more in the workspace's map, for
  one file of seventy lines. **A copy of the helper in each crate**: four copies that drift.
  **A crate of property testing or fuzzing**: ADR-0096 asks for no dependency.

## Consequences

- A game no longer panics on a light, a cooked file, a document or a scene file that a disk damaged
  or a player edited: it gets an error that names the file, and an asset refused is not drawn.
- Behaviour that changes for users: `Renderer::render` leaves out lights where it panicked;
  `GltfFile::open` takes the directory of sources; a manifest's names are restricted; a save holds
  about 22 000 entities with a name and a transform, by its values, where 16 MiB held 57 000
  (`World::save` says it); a scene file whose placements copy more than 256 MiB is refused.
- A cook that changes nothing reads every cooked file it reuses, at a cost that the rustdoc of
  `slime_cook::cook` measures.
- Damage that leaves a file readable, texels or numbers changed within their ranges, is not found:
  it draws wrong. Checksums may come later beside the rule, with archives, as ADR-0096 says.
- Every new reader comes with its damaged-input tests, through the shared helper.
