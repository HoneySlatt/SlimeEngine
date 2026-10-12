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
    /// [`GfxError::Shader`] if the source does not compile. Its message comes from the compiler
    /// and shows the line and column of each error.
    pub fn create_shader_module(&self, label: &str, wgsl: &str) -> Result<ShaderModule, GfxError> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
        match pollster::block_on(scope.pop()) {
            None => Ok(ShaderModule { module }),
            Some(error) => Err(GfxError::Shader {
                label: label.to_owned(),
                message: error.to_string().trim_end().to_owned(),
            }),
        }
    }
}
