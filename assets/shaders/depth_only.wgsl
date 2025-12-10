// Minimal depth-only shader used for outline meshes that should write depth
// (so they cast shadows) but not render any visible color in the main pass.

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip}

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(vertex.instance_index),
        vec4<f32>(vertex.position, 1.0),
    );
    return out;
}

@fragment
fn fragment(_mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Main pass: output doesn't matter because the pipeline will disable color writes
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

// Prepass vertex (same as main vertex)
@vertex
fn prepass_vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = mesh_position_local_to_clip(
        get_world_from_local(vertex.instance_index),
        vec4<f32>(vertex.position, 1.0),
    );
    return out;
}

// Prepass fragment: don't discard — writing depth is what we want
@fragment
fn prepass_fragment(_mesh: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
