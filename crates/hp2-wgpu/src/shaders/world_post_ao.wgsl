// Screen-space ambient occlusion from the world depth buffer (appended to
// world_post.wgsl when enabled). DEPTH_TEXTURE is texture_depth_2d or
// texture_depth_multisampled_2d; textureLoad's last argument is the mip level
// or sample index, 0 for both.
@group(0) @binding(2) var depth: DEPTH_TEXTURE;

// The same far plane as the scene that produced this depth texture.
@group(0) @binding(3) var<uniform> z_far: f32;
const Z_NEAR: f32 = 1.0;
const RADIUS: f32 = 40.0;

fn view_z(coord: vec2i, size: vec2i) -> f32 {
    let d = textureLoad(depth, clamp(coord, vec2i(0), size - vec2i(1)), 0);
    return Z_NEAR * z_far / (z_far - d * (z_far - Z_NEAR));
}

fn ambient_occlusion(uv: vec2f) -> f32 {
    let size = vec2i(textureDimensions(depth));
    let coord = vec2i(uv * vec2f(size));
    let d = textureLoad(depth, coord, 0);
    if d >= 0.9999 {
        return 1.0;
    }
    let z = view_z(coord, size);
    // Projected radius for a ~90 degree horizontal field of view.
    let focal = f32(size.x) * 0.5;
    let radius_px = clamp(RADIUS * focal / z, 2.0, f32(size.y) * 0.08);
    // Interleaved gradient noise rotates the kernel per pixel.
    let noise = fract(52.9829189 * fract(dot(vec2f(coord), vec2f(0.06711056, 0.00583715))));
    var occlusion = 0.0;
    for (var i = 0; i < 8; i++) {
        let t = (f32(i) + 0.5) / 8.0;
        let angle = (f32(i) * 2.3999632) + noise * 6.2831853;
        let offset = vec2f(cos(angle), sin(angle)) * radius_px * sqrt(t);
        let sample_z = view_z(coord + vec2i(offset), size);
        let delta = z - sample_z;
        // Closer neighbours occlude; far-away foreground (> 2 radii) does not.
        occlusion += clamp((delta - 2.0) / RADIUS, 0.0, 1.0) * select(0.0, 1.0, delta < RADIUS * 2.0);
    }
    return 1.0 - 0.55 * (occlusion / 8.0);
}
