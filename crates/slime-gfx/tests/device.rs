//! Opening the GPU without a window, on whatever adapter the machine offers: lavapipe when
//! `VK_DRIVER_FILES` points at it (as `scripts/check.sh` does), WARP on Windows machines without
//! a GPU (as in CI), the machine's GPU otherwise.

#[test]
fn opens_a_device_and_runs_work_without_a_window() {
    let gpu = slime_gfx::Gpu::new()
        .expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    eprintln!("adapter: {}", gpu.adapter_name());
    assert!(!gpu.adapter_name().is_empty());
    gpu.wait_idle().expect("the GPU finishes its work");
}
