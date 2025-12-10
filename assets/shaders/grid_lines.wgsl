#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::view_transformations::position_world_to_clip

struct LineMaterial {
    color: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: LineMaterial;

/// This shader generates a grid of lines on the XZ plane.
@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
) -> VertexOutput {
    // The number of lines to draw along each axis.
    let line_count_per_axis = 11u;
    // The total number of lines is twice the number per axis (for horizontal and vertical lines).
    let total_lines = line_count_per_axis * 2u;
    // Each line has two vertices.
    let vertices_per_line = 2u;

    // Determine which line this vertex belongs to.
    let line_index = vertex_index / vertices_per_line;
    // Determine if this is the start or end vertex of the line.
    let point_index = vertex_index % vertices_per_line;

    // The size of the grid.
    let grid_size = 10.0;
    let half_grid_size = grid_size / 2.0;

    var pos = vec3<f32>(0.0, 0.0, 0.0);

    // Generate horizontal and vertical lines.
    if (line_index < line_count_per_axis) {
        // Horizontal lines (along the X-axis).
        let z = f32(line_index) * grid_size / f32(line_count_per_axis - 1u) - half_grid_size;
        pos = vec3<f32>(f32(point_index) * grid_size - half_grid_size, 0.0, z);
    } else {
        // Vertical lines (along the Z-axis).
        let x = f32(line_index - line_count_per_axis) * grid_size / f32(line_count_per_axis - 1u) - half_grid_size;
        pos = vec3<f32>(x, 0.0, f32(point_index) * grid_size - half_grid_size);
    }

    var out: VertexOutput;
    out.world_position = vec4<f32>(pos, 1.0);
    out.position = position_world_to_clip(out.world_position.xyz);
    return out;
}


@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    return material.color;
}