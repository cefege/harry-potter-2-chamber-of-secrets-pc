// World-only post effects, applied once when the 3D world is composited into the
// frame (UI/HUD drawn afterwards is untouched). draw.rs prepends the `const`
// feature switches and appends `ambient_occlusion` (real or a stub returning 1).
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

fn tap(uv: vec2f) -> vec3f {
    return textureSampleLevel(source, source_sampler, uv, 0.0).rgb;
}

fn luma(c: vec3f) -> f32 {
    return dot(c, vec3f(0.299, 0.587, 0.114));
}

// FXAA 3.11 "console" variant: one directional blur along the local edge.
fn fxaa(uv: vec2f, px: vec2f) -> vec3f {
    let center = tap(uv);
    let l_nw = luma(tap(uv + vec2f(-1.0, -1.0) * px));
    let l_ne = luma(tap(uv + vec2f(1.0, -1.0) * px));
    let l_sw = luma(tap(uv + vec2f(-1.0, 1.0) * px));
    let l_se = luma(tap(uv + vec2f(1.0, 1.0) * px));
    let l_m = luma(center);
    let l_min = min(l_m, min(min(l_nw, l_ne), min(l_sw, l_se)));
    let l_max = max(l_m, max(max(l_nw, l_ne), max(l_sw, l_se)));
    if l_max - l_min < max(0.0312, l_max * 0.125) {
        return center;
    }
    var dir = vec2f(-((l_nw + l_ne) - (l_sw + l_se)), (l_nw + l_sw) - (l_ne + l_se));
    let reduce = max((l_nw + l_ne + l_sw + l_se) * (0.25 / 8.0), 1.0 / 128.0);
    let scale = 1.0 / (min(abs(dir.x), abs(dir.y)) + reduce);
    dir = clamp(dir * scale, vec2f(-8.0), vec2f(8.0)) * px;
    let a = 0.5 * (tap(uv + dir * (1.0 / 3.0 - 0.5)) + tap(uv + dir * (2.0 / 3.0 - 0.5)));
    let b = a * 0.5 + 0.25 * (tap(uv - dir * 0.5) + tap(uv + dir * 0.5));
    let l_b = luma(b);
    if l_b < l_min || l_b > l_max {
        return a;
    }
    return b;
}

// Contrast-adaptive sharpening (AMD CAS, simplified): returns the detail to add.
fn sharpen_delta(uv: vec2f, px: vec2f) -> vec3f {
    let c = tap(uv);
    let n = tap(uv + vec2f(0.0, -px.y));
    let s = tap(uv + vec2f(0.0, px.y));
    let w = tap(uv + vec2f(-px.x, 0.0));
    let e = tap(uv + vec2f(px.x, 0.0));
    let mn = min(c, min(min(n, s), min(w, e)));
    let mx = max(c, max(max(n, s), max(w, e)));
    let amp = sqrt(clamp(min(mn, vec3f(1.0) - mx) / max(mx, vec3f(1.0e-4)), vec3f(0.0), vec3f(1.0)));
    let weight = -amp * 0.125;
    let sharpened = (c + weight * (n + s + w + e)) / (vec3f(1.0) + 4.0 * weight);
    return sharpened - c;
}

// Soft glow from bright areas: two rings of taps, scaled with the resolution.
fn bloom(uv: vec2f, px: vec2f, dims: vec2f) -> vec3f {
    let scale = max(dims.y / 480.0, 1.0);
    var glow = vec3f(0.0);
    for (var i = 0; i < 12; i++) {
        let angle = f32(i) * 0.5235988;
        let ring = select(3.0, 7.0, (i & 1) != 0);
        let offset = vec2f(cos(angle), sin(angle)) * ring * scale * px;
        let c = tap(uv + offset);
        glow += max(c - vec3f(0.7), vec3f(0.0)) * select(1.0, 0.6, (i & 1) != 0);
    }
    return glow * (0.6 / 9.6);
}

// Slightly richer colour and contrast; highlights above 1 (HDR) are kept.
fn grade(c: vec3f) -> vec3f {
    let l = luma(c);
    var g = max(mix(vec3f(l), c, 1.12), vec3f(0.0));
    let low = clamp(g, vec3f(0.0), vec3f(1.0));
    let curve = low * low * (vec3f(3.0) - 2.0 * low);
    g = g + (curve - low) * 0.22;
    return g;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let dims = vec2f(textureDimensions(source));
    let px = 1.0 / dims;
    let alpha = textureSampleLevel(source, source_sampler, in.uv, 0.0).a;
    var color = tap(in.uv);
    if FXAA {
        color = fxaa(in.uv, px);
    }
    if SHARPEN {
        color = color + sharpen_delta(in.uv, px);
    }
    if AMBIENT_OCCLUSION {
        color = color * ambient_occlusion(in.uv);
    }
    if BLOOM {
        color = color + bloom(in.uv, px, dims);
    }
    if COLOR_GRADING {
        color = grade(color);
    }
    return vec4f(color, alpha);
}
