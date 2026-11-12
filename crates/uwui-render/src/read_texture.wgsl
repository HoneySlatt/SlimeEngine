// The canvas's store read from a texture of `Rgba32Uint`, a word a texel, row after row, where the
// device cannot read a storage buffer in both stages, as WebGL2: appended to `canvas.wgsl` by
// `canvas.rs`.

@group(0) @binding(1) var store: texture_2d<u32>;

// The texture's width, in texels: `STORE_ROW` in `canvas.rs`, WebGL2's smallest largest texture.
const STORE_ROW: u32 = 2048u;

// The word `at` of the store.
fn word(at: u32) -> vec4<u32> {
    return textureLoad(store, vec2<u32>(at % STORE_ROW, at / STORE_ROW), 0);
}
