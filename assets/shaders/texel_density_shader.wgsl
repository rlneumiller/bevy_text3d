// =============================================================================
// Texel Density Fragment Shader for TexelDensityMaterial
// =============================================================================
//
// PURPOSE:
// This fragment shader implements texel density visualization for 3D meshes.
// Texel density refers to how many texels (texture pixels) are mapped per unit
// of world space on a mesh surface. This is crucial for optimizing texture usage
// and ensuring consistent visual quality across different mesh LODs.
//
// VISUALIZATION METHOD:
// - Low density (few texels per world unit): Red coloring
// - Medium density: Yellow coloring
// - High density (many texels per world unit): Green coloring
//
// The shader overlays a world-space checker pattern to help identify texture
// stretching and compression artifacts.
//
// KEY FEATURES:
// 1. World-space texel density calculation using fwidth() on both UV and world position
// 2. Smooth color interpolation between density bands
// 3. Triplanar world-space checker pattern overlay
// 4. Seamless blending across mesh faces
//
// =============================================================================

// IMPORTANT: The VertexOutput struct must match Bevy's default mesh vertex shader outputs:
// - @location(0) world_position: vec4<f32> - World-space position (interpolated)
// - @location(1) world_normal: vec3<f32> - World-space normal (interpolated)
// - @location(2) uv: vec2<f32> - Texture coordinates (interpolated)
// This ensures compatibility with the custom vertex shader pipeline.

struct VertexOutput {
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

// =============================================================================
// Checker Pattern Generator
// =============================================================================
// Creates a procedural checkerboard pattern in UV space.
// Returns 0.0 for dark squares, 1.0 for light squares.
// Used for the world-space overlay pattern.
fn checker(uv: vec2<f32>, scale: f32) -> f32 {
    let uv_scaled = uv * scale;
    let fx = fract(uv_scaled.x);
    let fy = fract(uv_scaled.y);
    let bx = step(0.5, fx);
    let by = step(0.5, fy);
    return abs(bx - by);
}

// =============================================================================
// Main Fragment Shader
// =============================================================================
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // =========================================================================
    // TEXEL DENSITY CALCULATION
    // =========================================================================
    // Calculate world-space texel density: UV change per world space unit.
    // This normalizes for camera distance and object size.

    let uv_deriv = fwidth(in.uv);           // UV change across screen pixels
    let world_deriv = fwidth(in.world_position.xyz);  // World position change across screen pixels

    // World-space density: UV change per unit world distance
    let world_space_density = length(uv_deriv) / (length(world_deriv) + 1e-6);

    // Scale for visualization - very low multiplier to spread the range
    let density = world_space_density * 0.5;

    // =========================================================================
    // COLOR MAPPING
    // =========================================================================
    // Map density values to colors using smooth interpolation:
    // 0.0-0.5: Red to Yellow (low to medium density)
    // 0.5-1.0: Yellow to Green (medium to high density)
    // 1.0-2.0: Green to Blue (high to very high density)
    // >2.0: Blue (extremely high density)

    let t0 = smoothstep(0.0, 0.5, density);  // Interpolation factor for first band
    let t1 = smoothstep(0.5, 1.0, density);  // Interpolation factor for second band
    let t2 = smoothstep(1.0, 2.0, density);  // Interpolation factor for third band

    let color0 = vec3<f32>(1.0, 0.0, 0.0);  // Red (low density)
    let color1 = vec3<f32>(1.0, 1.0, 0.0);  // Yellow (medium density)
    let color2 = vec3<f32>(0.0, 1.0, 0.0);  // Green (high density)
    let color3 = vec3<f32>(0.0, 0.0, 1.0);  // Blue (very high density)

    // Interpolate between color bands
    let base_color = mix(mix(mix(color0, color1, t0), color2, t1), color3, t2);

    // =========================================================================
    // WORLD-SPACE CHECKER PATTERN OVERLAY
    // =========================================================================
    // Use triplanar mapping to create a seamless checker pattern in world space.
    // This avoids UV seam artifacts that would occur with traditional UV mapping.

    let checker_scale: f32 = 4.0;  // Pattern frequency in world units
    let world_normal = normalize(in.world_normal);

    // Calculate triplanar blending weights based on face normal.
    // Faces perpendicular to an axis get full weight for that plane.
    var blend_weights = abs(world_normal);

    // Sharpen the weights to reduce blurring at edges (power of 3).
    blend_weights = pow(blend_weights, vec3<f32>(3.0));

    // Normalize to ensure weights sum to 1.0
    blend_weights = blend_weights / (blend_weights.x + blend_weights.y + blend_weights.z);

    // Sample checker pattern from each of the 3 world planes:
    // X-aligned plane uses YZ coordinates, Y-aligned uses XZ, Z-aligned uses XY
    let checker_x = checker(in.world_position.xyz.yz, checker_scale);
    let checker_y = checker(in.world_position.xyz.xz, checker_scale);
    let checker_z = checker(in.world_position.xyz.xy, checker_scale);

    // Blend the three checker samples based on face orientation
    let checker_val = checker_x * blend_weights.x +
                      checker_y * blend_weights.y +
                      checker_z * blend_weights.z;

    // =========================================================================
    // FINAL COLOR COMPOSITION
    // =========================================================================
    // Blend the density-based base color with the checker pattern.
    // Lower intensity (0.3) makes the checker subtle but visible.
    let checker_intensity = 0.3;
    let final_color = mix(base_color, vec3<f32>(checker_val), checker_intensity);

    return vec4<f32>(final_color, 1.0);
}