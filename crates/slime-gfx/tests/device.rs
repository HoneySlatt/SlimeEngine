//! Opening the GPU without a window, on whatever adapter the machine offers: lavapipe when
//! `VK_DRIVER_FILES` points at it (as `scripts/check.sh` does), WARP on Windows machines without
//! a GPU (as in CI), the machine's GPU otherwise; and failing to, without any graphics API. An object
//! of one device used with another panics (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`).

use std::process::Command;

use slime_gfx::GfxError;

#[test]
fn opens_a_device_and_runs_work_without_a_window() {
    let gpu = slime_gfx::Gpu::new()
        .expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    eprintln!("adapter: {}", gpu.adapter_name());
    assert!(!gpu.adapter_name().is_empty());
    gpu.wait_idle().expect("the GPU finishes its work");
}

#[test]
fn an_object_used_with_another_device_than_its_own_panics() {
    let open = || {
        slime_gfx::Gpu::new()
            .expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
    };
    let (mine, other) = (open(), open());
    let buffer = mine
        .create_uniform_buffer("mine", &[0; 16])
        .expect("the GPU has memory for it");
    // A clone is the same device.
    mine.clone().write_uniform_buffer(&buffer, &[1; 16]);
    let used = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        other.write_uniform_buffer(&buffer, &[2; 16]);
    }));
    let message = used.expect_err("another device's object refused");
    let message = message
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or_default();
    assert!(
        message.contains("a uniform buffer of another GPU device"),
        "{message}"
    );
}

/// wgpu reads `WGPU_BACKEND` from the environment, which a test changes safely only for a process
/// of its own: the test runs itself again, asking for a graphics API that this build does not have.
#[test]
fn no_graphics_api_means_no_adapter() {
    const CHILD_ENV: &str = "SLIME_GFX_TEST_CHILD";
    if std::env::var_os(CHILD_ENV).is_some() {
        let result = slime_gfx::Gpu::new();
        assert!(
            matches!(result, Err(GfxError::NoAdapter(_))),
            "no graphics API, no adapter: {result:?}"
        );
        return;
    }
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", "no_graphics_api_means_no_adapter"])
        .env(CHILD_ENV, "1")
        .env("WGPU_BACKEND", "gl")
        .output()
        .expect("child test process runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "the child failed: {}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_largest_texture_and_buffer_are_the_adapter_s_at_least_the_floor_s() {
    let gpu = slime_gfx::Gpu::new()
        .expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    assert!(gpu.max_texture_dimension() >= 8192);
    assert!(gpu.max_buffer_size() >= 256 << 20);
    assert!(gpu.max_storage_buffer_size() >= 128 << 20);
}
