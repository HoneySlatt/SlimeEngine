//! Kinds of materials: the ways of lighting a surface that a renderer knows by registration
//! (`docs/adr/0111-the-frame-contract.md`).

use slime_gfx::{BindGroupLayoutEntry, BindingType, ShaderStages};
use slime_reflect::{Reflect, Registry, Value};

use crate::material::{
    AlphaMode, MATERIAL_TEXTURE_SLOTS, MATERIAL_VALUES_SIZE, MaterialInner, MaterialValues,
    UNLIT_VALUES_SIZE, UnlitValues,
};
use crate::shaders::ShaderSources;

/// A way of lighting a surface, a *kind* of material: it gives the renderer the shader of its
/// passes, the bindings of its materials' group, whether it draws in the shadow passes, and the
/// type of its materials' parameters, described by reflection, so that an editor shows and edits
/// those of any kind, and it converts its materials' parameters into what its shader reads. A
/// renderer registers the kinds it draws when it is made, and each material is of one of them.
///
/// The physically based kind and the unlit one are registered (ADR-0129): Stage 9a sets what a kind
/// gives the engine, with its method for many lights, and lets a game register kinds of its own
/// (ADR-0111).
pub(crate) trait MaterialKind: Send + Sync + std::fmt::Debug {
    /// Its name, which [`Material::kind`](crate::Material::kind) gives.
    fn name(&self) -> &'static str;

    /// The file among `sources` that holds its passes, and its source: the entry points `vs_main`
    /// and `fs_main` draw it in the scene pass, and, for a kind that casts shadows, `vs_shadow` and
    /// `fs_shadow` in the shadow passes; they read the renderer's groups 0, 2 and 3, as
    /// `shader.wgsl` declares them, and the material's group at 1.
    fn shader<'s>(&self, sources: &'s ShaderSources) -> (&'static str, &'s str);

    /// The bindings of its materials' group.
    fn bindings(&self) -> Vec<BindGroupLayoutEntry>;

    /// Whether the surfaces of its materials cast shadows, so that the shadow passes draw them.
    fn casts_shadows(&self) -> bool;

    /// Whether its materials may be blended ([`AlphaMode::Blend`]), so that the scene pass has
    /// pipelines that lay them over what is behind them.
    fn blends(&self) -> bool;

    /// Registers the type of its materials' parameters in `registry`, and gives its name.
    fn describe(&self, registry: &mut Registry) -> String;

    /// What its shader reads at binding 0 of `material`'s group, made from `parameters`, a value of
    /// its parameters' type; or why it refuses them for that material.
    fn pack(&self, material: &MaterialInner, parameters: &Value) -> Result<Vec<u8>, String>;
}

/// glTF's metallic-roughness material (ADR-0035): the kind of every material that
/// [`Renderer::create_material`](crate::Renderer::create_material) and the loads of cooked data
/// make, whose parameters are [`MaterialValues`].
#[derive(Debug)]
pub(crate) struct PhysicallyBased;

impl MaterialKind for PhysicallyBased {
    fn name(&self) -> &'static str {
        "physically based"
    }

    fn shader<'s>(&self, sources: &'s ShaderSources) -> (&'static str, &'s str) {
        ("shader.wgsl", &sources.scene)
    }

    fn bindings(&self) -> Vec<BindGroupLayoutEntry> {
        // The material's values, then each texture slot: a texture and its sampler.
        let mut bindings = vec![BindGroupLayoutEntry {
            binding: 0,
            visible_to: ShaderStages::Fragment,
            ty: BindingType::UniformBuffer {
                dynamic_offset: false,
                size: MATERIAL_VALUES_SIZE as u64,
            },
        }];
        for slot in 0..MATERIAL_TEXTURE_SLOTS {
            for (binding, ty) in [
                (1 + 2 * slot, BindingType::Texture),
                (2 + 2 * slot, BindingType::Sampler),
            ] {
                bindings.push(BindGroupLayoutEntry {
                    binding,
                    visible_to: ShaderStages::Fragment,
                    ty,
                });
            }
        }
        bindings
    }

    fn casts_shadows(&self) -> bool {
        true
    }

    fn blends(&self) -> bool {
        false
    }

    fn describe(&self, registry: &mut Registry) -> String {
        registry.register::<MaterialValues>();
        MaterialValues::type_name()
    }

    fn pack(&self, material: &MaterialInner, parameters: &Value) -> Result<Vec<u8>, String> {
        let values = MaterialValues::from_value(parameters).map_err(|error| error.to_string())?;
        if matches!(values.alpha_mode, AlphaMode::Mask { .. }) != material.masked {
            let stays = if material.masked { "a mask" } else { "opaque" };
            return Err(format!("its alpha mode stays {stays}"));
        }
        Ok(values.bytes(material.normal_texture))
    }
}

/// A color shown as it is, whatever lights the scene and whatever its exposure (ADR-0129): the kind
/// of the materials that
/// [`Renderer::create_unlit_material`](crate::Renderer::create_unlit_material) makes, whose
/// parameters are [`UnlitValues`].
#[derive(Debug)]
pub(crate) struct Unlit;

impl MaterialKind for Unlit {
    fn name(&self) -> &'static str {
        "unlit"
    }

    fn shader<'s>(&self, sources: &'s ShaderSources) -> (&'static str, &'s str) {
        ("unlit.wgsl", &sources.unlit)
    }

    fn bindings(&self) -> Vec<BindGroupLayoutEntry> {
        // Its values, then its texture and sampler.
        [
            BindingType::UniformBuffer {
                dynamic_offset: false,
                size: UNLIT_VALUES_SIZE as u64,
            },
            BindingType::Texture,
            BindingType::Sampler,
        ]
        .into_iter()
        .zip(0..)
        .map(|(ty, binding)| BindGroupLayoutEntry {
            binding,
            visible_to: ShaderStages::Fragment,
            ty,
        })
        .collect()
    }

    fn casts_shadows(&self) -> bool {
        false
    }

    fn blends(&self) -> bool {
        true
    }

    fn describe(&self, registry: &mut Registry) -> String {
        registry.register::<UnlitValues>();
        UnlitValues::type_name()
    }

    fn pack(&self, material: &MaterialInner, parameters: &Value) -> Result<Vec<u8>, String> {
        let values = UnlitValues::from_value(parameters).map_err(|error| error.to_string())?;
        let mode = (
            matches!(values.alpha_mode, AlphaMode::Mask { .. }),
            values.alpha_mode == AlphaMode::Blend,
        );
        if mode != (material.masked, material.blended) {
            return Err("its alpha mode stays the one it was made with".to_owned());
        }
        Ok(values.bytes())
    }
}
