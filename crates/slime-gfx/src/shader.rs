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
    /// shows the line and column of each error; or if the graphics driver rejects the compiled
    /// module, with the driver's message.
    pub fn create_shader_module(&self, label: &str, wgsl: &str) -> Result<ShaderModule, GfxError> {
        let (module, error) = self.catch_errors(|| {
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(label),
                    source: wgpu::ShaderSource::Wgsl(wgsl.into()),
                })
        });
        match error {
            None => Ok(ShaderModule { module }),
            Some(error) => Err(GfxError::Shader {
                label: label.to_owned(),
                message: error.to_string().trim_end().to_owned(),
            }),
        }
    }
}
