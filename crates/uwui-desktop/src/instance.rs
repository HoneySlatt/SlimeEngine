//! The wgpu instance that the host, and the blank window beside it, open their device on: the
//! graphics API where `WGPU_BACKEND` names none, Direct3D 12 on Windows, and there its shader
//! compiler, the DXC that lies beside the program, else FXC
//! (`docs/adr/0317-m6d7c-cut-in-three-windows-on-windows-sys-its-frames-on-the-displays-vertical-blank-direct3d-12-by-default.md`,
//! `docs/adr/0066-shipping-dxc-beside-the-programs.md`). Its choice of the compiler is the engine's
//! own code, `crates/slime-gfx/src/gpu.rs`, taken under uwui's licence, the owner's go of
//! 2026-11-25: uwui does not depend on the engine; the two part at M6d.11.

/// The graphics API where `WGPU_BACKEND` names none: Direct3D 12 on Windows, as the engine's, whose
/// swap chain alone gives DXGI's statistics; elsewhere wgpu's first choice of the platform's own.
#[cfg(windows)]
const DEFAULT_BACKENDS: wgpu::Backends = wgpu::Backends::DX12;
#[cfg(not(windows))]
const DEFAULT_BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY;

/// The DXC library's file name, which Windows programs ship beside them.
const DXC_LIBRARY: &str = "dxcompiler.dll";

/// The instance: `WGPU_BACKEND`'s API, else the platform's default; its flags from the build and
/// the environment; on Direct3D 12, its shader compiler as [`dx12_compiler`] chooses it.
pub(crate) fn instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::from_env().unwrap_or(DEFAULT_BACKENDS);
    descriptor.flags = wgpu::InstanceFlags::from_build_config().with_env();
    if cfg!(windows) && descriptor.backends.contains(wgpu::Backends::DX12) {
        let requested = wgpu::Dx12Compiler::from_env();
        let asked_fxc = matches!(requested, Some(wgpu::Dx12Compiler::Fxc));
        let compiler = dx12_compiler(requested, dxc_beside_program());
        match &compiler {
            wgpu::Dx12Compiler::DynamicDxc { dxc_path } => {
                tracing::info!(path = %dxc_path, "Direct3D 12 compiles shaders with DXC");
            }
            _ if asked_fxc => {
                tracing::info!("Direct3D 12 compiles shaders with FXC, as WGPU_DX12_COMPILER asks");
            }
            _ => tracing::warn!(
                "Direct3D 12 compiles shaders with FXC, whose code runs slower than DXC's: no \
                 {DXC_LIBRARY} lies beside the program (ADR-0066)"
            ),
        }
        descriptor.backend_options.dx12.shader_compiler = compiler;
    }
    wgpu::Instance::new(descriptor)
}

/// The shader compiler of Direct3D 12: what `WGPU_DX12_COMPILER` asks for, given as `requested`,
/// if it is FXC or DXC wherever Windows finds it; otherwise DXC loaded from `beside`, the full path
/// of the `dxcompiler.dll` beside the program if there is one, so that Windows loads that very file,
/// and FXC without it. wgpu's other choices are not taken: `Auto` loads any `dxcompiler.dll` that
/// Windows' search finds, on `PATH` too, whatever its version; `StaticDxc` needs a feature of wgpu
/// that uwui does not build.
fn dx12_compiler(
    requested: Option<wgpu::Dx12Compiler>,
    beside: Option<String>,
) -> wgpu::Dx12Compiler {
    match (requested, beside) {
        (Some(asked @ (wgpu::Dx12Compiler::Fxc | wgpu::Dx12Compiler::DynamicDxc { .. })), _) => {
            asked
        }
        (_, Some(dxc_path)) => wgpu::Dx12Compiler::DynamicDxc { dxc_path },
        (_, None) => wgpu::Dx12Compiler::Fxc,
    }
}

/// The full path of the `dxcompiler.dll` beside the running program, where there is one and its
/// path is valid Unicode, as wgpu takes it.
fn dxc_beside_program() -> Option<String> {
    let path = std::env::current_exe().ok()?.with_file_name(DXC_LIBRARY);
    if !path.is_file() {
        return None;
    }
    path.into_os_string().into_string().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(compiler: &wgpu::Dx12Compiler) -> String {
        match compiler {
            wgpu::Dx12Compiler::Fxc => "fxc".to_owned(),
            wgpu::Dx12Compiler::DynamicDxc { dxc_path } => format!("dxc {dxc_path}"),
            wgpu::Dx12Compiler::StaticDxc => "static".to_owned(),
            _ => "auto".to_owned(),
        }
    }

    #[test]
    fn the_dxc_beside_the_program_compiles_unless_the_environment_asks_otherwise() {
        let beside = || Some("C:\\program\\dxcompiler.dll".to_owned());
        assert_eq!(
            named(&dx12_compiler(None, beside())),
            "dxc C:\\program\\dxcompiler.dll"
        );
        // Without it, FXC, never any DXC that Windows' search finds.
        assert_eq!(named(&dx12_compiler(None, None)), "fxc");
        assert_eq!(
            named(&dx12_compiler(Some(wgpu::Dx12Compiler::Auto), None)),
            "fxc"
        );
        assert_eq!(
            named(&dx12_compiler(
                Some(wgpu::Dx12Compiler::StaticDxc),
                beside()
            )),
            "dxc C:\\program\\dxcompiler.dll"
        );
        // Asked for by the environment.
        assert_eq!(
            named(&dx12_compiler(Some(wgpu::Dx12Compiler::Fxc), beside())),
            "fxc"
        );
        assert_eq!(
            named(&dx12_compiler(
                Some(wgpu::Dx12Compiler::default_dynamic_dxc()),
                None
            )),
            "dxc dxcompiler.dll"
        );
    }

    #[test]
    fn windows_opens_direct3d_12_where_the_environment_names_no_api() {
        assert_eq!(
            DEFAULT_BACKENDS == wgpu::Backends::DX12,
            cfg!(windows),
            "{DEFAULT_BACKENDS:?}"
        );
    }
}
