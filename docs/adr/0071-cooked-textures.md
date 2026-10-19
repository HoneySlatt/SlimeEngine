# 0071. Cooked textures

- Status: Accepted; amends [0032](0032-mipmaps-computed-at-texture-creation.md) (who computes the
  levels of a texture); amended by [0102](0102-a-damaged-file-gives-an-error.md) (who reads KTX2
  files, and the cooked files reused)
- Date: 2026-10-15

## Context

The asset pipeline cooks images into textures compressed for the GPU, BC7 and BC5 in KTX2 files
([ADR-0069](0069-the-asset-pipeline.md)), so that *Hunna Wood*'s forty or so textures fit in a
quarter of the memory that 8-bit texels take. Until now, the GPU layer computed the levels of
detail of every texture from its full-size image, on the CPU, when it was created
([ADR-0032](0032-mipmaps-computed-at-texture-creation.md)): about 70 ms for 2048 × 2048 texels on
the thread that asks. A block-compressed texture cannot be filtered that way: its levels must be
computed before they are compressed.

An image's use decides how it should be filtered and stored: colors are sRGB-encoded and average in
linear light; data such as occlusion, roughness and metalness average as stored; a normal map holds
vectors, which average to shorter vectors that shading needs of length one again. The image alone
does not say its use: glTF's materials do, slot by slot.

The encoder chosen, `block_compression`, a port of Intel's ISPC encoders to Rust, offers settings
from "ultra fast" to "slow". Measured on 2048 × 2048 texels of a noisy test image, on the 16
threads of the development machine:

| BC7 settings | Time | Peak signal-to-noise ratio |
|---|---|---|
| ultra fast | 38 ms | 34.6 dB |
| very fast | 0.55 s | 54.4 dB |
| fast | 0.86 s | 55.3 dB |
| basic | 1.99 s | 55.3 dB |

With "basic", every level of a texture of 4096 × 4096 texels took 10.2 s; with "fast", 4.9 s.

## Decision

**Roles.** A texture is cooked for one *role*, which sets its filter and its encoding:

| Role | Levels averaged | Encoding, sides multiples of 4 | Otherwise |
|---|---|---|---|
| Color (base color, emission) | In linear light, then sRGB-encoded | BC7 with sRGB | 8-bit texels, sRGB |
| Data (occlusion, roughness, metalness) | As stored | BC7 | 8-bit texels |
| Normal (normal maps) | As vectors, made of length one again | BC5: x and y | 8-bit texels |

An image the cooker meets alone is cooked as colors, under the image's own ID: a game loads it by
that ID. The materials of glTF files ask for the other roles with the importer of glTF (M4a.4),
under IDs derived from the image's.

**Levels.** The cooker computes every level down to one texel with the GPU layer's footprints
(each texel of a level the weighted average of the texels it covers in the level before, odd sides
included), the role's way. A level smaller than a block of 4 × 4 repeats its last row and column to
fill one. Textures made in code keep their levels computed by `Gpu::create_texture`, as ADR-0032
decided.

**Encoding.** BC7 with the "fast" settings, with alpha when a texel of the image is not opaque:
the same error as "basic" on the test image in less than half the time. The blocks of each level
are encoded in bands of rows on every thread the machine offers; the bytes do not depend on their
number. A texture whose sides are not multiples of 4 stays in 8-bit texels: the GPU takes blocks
only for such sizes.

**Normals in two channels.** The renderer rebuilds the z of every normal texture from its x and
y, as glTF asks normals of length one, and never reads its blue: a BC5 texture, which has no blue,
serves as a texture of 8-bit texels does.

**The file.** One KTX2 file per texture, holding every level, the smallest first, each at a
multiple of 16 bytes (4 for texels), with the data format descriptor of its format and no
supercompression. Its name is the BLAKE3 hash of all that made it: the role, the asset's ID, the
image's bytes and the cooker's version, `TEXTURE_COOKER_VERSION`, raised whenever the same image
would give other bytes. A file of that name in the output is not cooked again.

**The manifest** of a directory of cooked data, version 1 of its binary format, lists each asset
with its kind, file, size, what it needs and its source. `CookedAssets` reads it, then the files a
game asks for by ID, on the calling thread; `Renderer::create_texture` uploads a cooked texture.

**A GPU without BC compression** refuses a compressed texture when it is created, with an error
that names it (`GfxError::UnsupportedTexture`): an engine without compressed textures still runs
there.

## Alternatives considered

- **The "basic" settings**: twice the time for no better blocks on the test image. **"Very
  fast"**: a third faster again, 0.9 dB further from the image. Photographs may differ: the
  settings are one line to change, with the cooker's version.
- **BC1, half the size of BC7, or BC3, the same size**: blocks further from the image, with
  visible banding in gradients. **BC7 for normal maps**: its channels share their endpoints, and normals lose more
  than with BC5's two independent channels.
- **8-bit texels for normal maps, z kept**: twice the memory of BC5, for a channel that the
  normal's length gives anyway.
- **Levels computed on the GPU when a texture is loaded**: no cooking time, but compressed levels
  cannot be computed that way, and every load pays.
- **Supercompressed KTX2 (Zstandard)**: smaller files on disk, another dependency, and a
  decompression at every load.

## Consequences

- A texture of 2048 × 2048 texels takes about a second to cook, every level, once: a second cook of
  the same image reuses its file.
- The renderer reads normal textures by x and y only; a normal texture whose texels are not of
  length one shades a little differently than before. The sample's image tests still pass, the
  largest difference 1 step on lavapipe.
- The cooked bytes are the same from one cook to the next on one platform, not across platforms:
  the sRGB curve is computed with the platform's mathematical functions.
- Files that no source makes any more stay in the output directory, until a command cleans them.
- ADR-0032's levels computed at creation remain for textures made in code; a cooked texture
  carries its own.
