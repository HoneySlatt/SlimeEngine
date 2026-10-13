//! Shader modules compiled from WGSL.

use crate::error::GfxError;
use crate::gpu::Gpu;

/// A compiled shader module, holding the entry points that pipelines use.
///
/// Clones refer to the same module.
#[derive(Clone, Debug)]
pub struct ShaderModule {
    pub(crate) module: wgpu::ShaderModule,
}

impl Gpu {
    /// Compiles WGSL source text into a shader module. `label` names it in error messages and in
    /// GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::Shader`] if the source does not compile, with the compiler's message, which
    /// shows the line and column of each error; or, on Vulkan, if wgpu cannot translate it to
    /// SPIR-V, with a generic message (wgpu's log has the cause). On Direct3D 12, the translation
    /// happens when a pipeline is created, so its failure is a [`GfxError::Pipeline`] from
    /// [`Gpu::create_render_pipeline`]. [`GfxError::DeviceLost`] once the device is lost
    /// ([`Gpu::check`]), including when creating the module loses it: on Vulkan, wgpu treats a
    /// driver that fails to create it as a lost device.
    pub fn create_shader_module(&self, label: &str, wgsl: &str) -> Result<ShaderModule, GfxError> {
        self.check()?;
        let (module, error) = self.catch_errors(|| {
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(label),
                    source: wgpu::ShaderSource::Wgsl(wgsl.into()),
                })
        })?;
        match error {
            None => Ok(ShaderModule { module }),
            Some(error) => Err(GfxError::Shader {
                label: label.to_owned(),
                message: error.to_string().trim_end().to_owned(),
            }),
        }
    }
}
