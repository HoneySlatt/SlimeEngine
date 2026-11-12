// The canvas's store read from a storage buffer, where both stages of the device can read one:
// appended to `canvas.wgsl` by `canvas.rs`.

@group(0) @binding(1) var<storage, read> store: array<vec4<u32>>;

// The word `at` of the store.
fn word(at: u32) -> vec4<u32> {
    return store[at];
}
