// Borrowed from: https://bevyengine.org/examples/shader_advanced/custom_vertex_attribute.rs
use bevy::{
    asset::Asset,
    math::Vec4,
    prelude::{AlphaMode, Handle, Image, Material, Mesh},
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, ColorWrites, CompareFunction, ShaderType, VertexFormat,
    },
    shader::ShaderRef,
};
use bevy_mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef};

pub const ATTRIBUTE_POSITION: MeshVertexAttribute =
    MeshVertexAttribute::new("Glyph_Vertex_Position", 988540917, VertexFormat::Float32x2);

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct GlyphMaterialUniform {
    pub params: Vec4,
}

impl GlyphMaterialUniform {
    pub fn with_smoothing(smoothing: f32) -> Self {
        Self {
            params: Vec4::new(smoothing, 0.0, 0.0, 0.0),
        }
    }

    pub fn smoothing(&self) -> f32 {
        self.params.x
    }
}

impl Default for GlyphMaterialUniform {
    fn default() -> Self {
        Self::with_smoothing(1.0)
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GlyphMaterial {
    #[uniform(0)]
    pub params: GlyphMaterialUniform,
    #[texture(1)]
    #[sampler(2)]
    pub sdf_texture: Handle<Image>,
}

impl Material for GlyphMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Path("shaders/text3d.wgsl".into())
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/text3d.wgsl".into())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        ShaderRef::Path("shaders/text3d_prepass.wgsl".into())
    }

    fn prepass_fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/text3d_prepass.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // TODO: store position/uv/color per char in SSBO, instead of per vertex
        let vertex_layout = layout.0.get_layout(&[
            ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(2),
        ])?;

        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;

        // Set entry points - use "vertex" and "fragment" for both main and prepass
        descriptor.vertex.entry_point = Some("vertex".into());
        if let Some(ref mut fragment) = descriptor.fragment {
            fragment.entry_point = Some("fragment".into());
        }

        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DepthOnlyMaterial {}

impl Material for DepthOnlyMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Path("shaders/depth_only.wgsl".into())
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/depth_only.wgsl".into())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        // Use our minimal depth-only vertex for prepass/shadow rendering.
        ShaderRef::Path("shaders/depth_only.wgsl".into())
    }

    fn prepass_fragment_shader() -> ShaderRef {
        // Use our minimal depth-only fragment for prepass/shadow rendering.
        ShaderRef::Path("shaders/depth_only.wgsl".into())
    }

    // Use default alpha mode (opaque)
    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        use bevy::render::render_resource::Face;

        // Disable color writes so this material only affects depth/prepass.
        if let Some(fragment) = descriptor.fragment.as_mut() {
            if let Some(target) = fragment.targets.get_mut(0) {
                if let Some(t) = target {
                    t.write_mask = ColorWrites::empty();
                }
            }
        }

        // Ensure depth writes are enabled and comparison is correct. Some drivers
        // can end up with a default state that doesn't write to depth for custom
        // materials unless explicitly set.
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = true;
            ds.depth_compare = CompareFunction::LessEqual;
        }

        // Enable backface culling so that when viewing the text from behind,
        // the back faces of the glyph profile mesh don't write depth and occlude the text.
        // When viewing from the front, front faces write depth (back faces are culled).
        // When viewing from behind, neither front nor back faces should occlude because:
        //   - Front faces are facing away (depth test fails)
        //   - Back faces are culled (don't render)
        // This ensures the invisible depth-only mesh doesn't block the text when viewed from behind.
        descriptor.primitive.cull_mode = Some(Face::Back);

        // The glyph profile meshes only provide a position attribute (Float32x3).
        // Ensure the vertex buffer layout matches that expectation.
        let vertex_layout = _layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?;
        descriptor.vertex.buffers = vec![vertex_layout];

        // Set entry points - use "vertex" and "fragment" for both main and prepass
        descriptor.vertex.entry_point = Some("vertex".into());
        if let Some(ref mut fragment) = descriptor.fragment {
            fragment.entry_point = Some("fragment".into());
        }

        Ok(())
    }
}
