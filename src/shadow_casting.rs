use bevy::{
    pbr::{ExtendedMaterial, MaterialPlugin},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ColorWrites},
    shader::ShaderRef,
};

/// An extension for StandardMaterial that disables color writes, making meshes "shadow-only"
/// while still using the robust PBR shadow pipeline.
#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct NoColorExt {}

impl bevy::pbr::MaterialExtension for NoColorExt {
    fn prepass_fragment_shader() -> ShaderRef {
        // Use default StandardMaterial prepass for shadow rendering
        ShaderRef::Default
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy_mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Only disable color writes in the main/forward pass, not in shadow/prepass
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.write_mask = ColorWrites::empty();
            }
        }
        // Ensure depth write is enabled for shadow casting
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_write_enabled = true;
        }
        // Ensure backface culling is disabled so flat glyph planes cast shadows from both sides.
        // TODO: Make this configurable
        descriptor.primitive.cull_mode = None;
        Ok(())
    }

    // Opaque mode ensures the material participates in shadow passes,
    // while ColorWrites::empty() makes it invisible in the color pass.
    fn alpha_mode() -> Option<bevy::render::alpha::AlphaMode> {
        Some(bevy::render::alpha::AlphaMode::Opaque)
    }
}

/// A shadow-only material based on StandardMaterial with NoColorExt.
/// This material disables color writes, making meshes invisible in the color pass
/// while still participating in depth and shadow passes.
pub type ShadowOnlyMaterial = ExtendedMaterial<StandardMaterial, NoColorExt>;

/// Registers [`ShadowOnlyMaterial`] with Bevy's renderer so it can be used like any other
/// `MeshMaterial3d`. This plugin enables shadow casting and prepass support required for
/// invisible shadow casters.
pub struct ShadowOnlyMaterialPlugin;

impl Plugin for ShadowOnlyMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShadowOnlyMaterial> {
            prepass_enabled: true,
            shadows_enabled: true,
            ..Default::default()
        });
    }
}

/// Convenience constructor for wrapping a [`StandardMaterial`] so it only contributes to
/// the shadow maps while remaining invisible in the main color pass.
pub fn create_shadow_only_material(base: StandardMaterial) -> ShadowOnlyMaterial {
    ShadowOnlyMaterial {
        base,
        extension: NoColorExt {},
    }
}

/// Bundle for spawning meshes that cast shadows without rendering any visible geometry in
/// the primary color pass. Useful for effects such as "detached" shadows.
#[derive(Bundle)]
pub struct ShadowOnlyMeshBundle {
    pub mesh: Mesh3d,
    pub material: MeshMaterial3d<ShadowOnlyMaterial>,
    pub transform: Transform,
}

impl ShadowOnlyMeshBundle {
    /// Creates a new bundle from a mesh handle and a [`ShadowOnlyMaterial`] handle.
    pub fn new(mesh: Handle<Mesh>, material: Handle<ShadowOnlyMaterial>) -> Self {
        Self {
            mesh: Mesh3d(mesh),
            material: MeshMaterial3d(material),
            transform: Transform::IDENTITY,
        }
    }

    /// Applies a local transform to the bundle so the caller can position or scale the caster.
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
}
