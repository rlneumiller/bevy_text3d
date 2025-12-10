// Prepass shader for Text3d material - no material bindings needed.

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

// Prepass vertex output - includes UV for fragment shader
struct PrepassVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Prepass vertex shader - outputs position and UV
@vertex
fn vertex(vertex: Vertex) -> PrepassVertexOutput {
    var out: PrepassVertexOutput;
    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(vertex.instance_index),
        vec4<f32>(vertex.position, 0.0, 1.0),
    );
    out.uv = vertex.uv;
    return out;
}

// Prepass fragment shader - write depth for all pixels (approximation for text shadows)
@fragment
fn fragment(
    mesh: PrepassVertexOutput,
) -> @location(0) vec4<f32> {
    return vec4(0.0, 0.0, 0.0, 1.0);
}