// World-only bloom. The small, thresholded kernel adds a restrained halo.
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
}

@vertex fn vs_main(@builtin(vertex_index) i: u32) -> VertexOutput {
    let x = select(-1.0, 1.0, (i & 1u) != 0u);
    let y = select(-1.0, 1.0, (i & 2u) != 0u);
    var out: VertexOutput;
    out.position = vec4f(x, y, 0.0, 1.0);
    out.uv = vec2f(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let base = textureSample(source, source_sampler, in.uv);
    let step = 2.0 / vec2f(textureDimensions(source));
    var glow = vec3f(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let c = textureSample(source, source_sampler, in.uv + vec2f(f32(x), f32(y)) * step).rgb;
            glow += max(c - vec3f(0.75), vec3f(0.0));
        }
    }
    return vec4f(base.rgb + glow * (0.35 / 9.0), base.a);
}
