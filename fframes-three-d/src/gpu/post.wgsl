// The post processing passes of the GPU renderer: ambient occlusion, depth of field, motion
// blur accumulation, bloom, and the final tone mapping and encoding. They follow
// `render/post.rs`.

struct Post {
    // focus distance, blur (pixels), max blur, tile size
    dof: vec4f,
    // threshold, knee, up weight, strength; in the encoding: mix (1 mixes the bloom in),
    // 1 / the total weight of the bloom levels, 0, strength
    bloom: vec4f,
    // exposure, tone mapping (0 linear, 1 aces, 2 neutral, 3 agx), has bloom, accumulate
    // weight
    encode: vec4f,
    // texel size of the source (1 / width, 1 / height), 0, 0
    texel: vec4f,
    // ambient occlusion: focal length (pixels at 1 m), radius (meters), strength, largest
    // radius (pixels)
    ao: vec4f,
}

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var src_sampler: sampler;
@group(0) @binding(3) var depth_key: texture_2d<f32>;
@group(0) @binding(4) var tiles: texture_2d<f32>;
// The bloom in the encoding pass, the half resolution frame in the depth of field.
@group(0) @binding(5) var aux_tex: texture_2d<f32>;
// The half resolution depth keys in the depth of field.
@group(0) @binding(6) var half_key: texture_2d<f32>;

struct FullscreenOut {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let uv = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullscreenOut;
    out.clip = vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2f(uv.x, 1.0 - uv.y);
    return out;
}

// The depth of field.

// The blur radius of a depth key (1 / distance, 0 for the background).
fn coc_of(key: f32) -> f32 {
    return min(post.dof.y * abs(1.0 - post.dof.x * key), post.dof.z);
}

// The largest blur in a tile of the depth buffer. The blur grows with the distance from the
// focus on both sides. So it is largest at the nearest or at the farthest point of the tile.
// Gathers find the key range of the tile, four keys per fetch.
@fragment
fn fs_tile(in: FullscreenOut) -> @location(0) vec4f {
    let tile = i32(post.dof.w);
    let origin = vec2i(in.clip.xy) * tile;
    let size = vec2f(textureDimensions(depth_key));
    var near = 0.0;
    var far = 1e30;
    for (var y = 0; y < tile; y += 2) {
        for (var x = 0; x < tile; x += 2) {
            // The center of a 2x2 block. The sampler clamps the blocks past the edge.
            let p = (vec2f(origin + vec2i(x, y)) + 1.0) / size;
            let keys = textureGather(0, depth_key, src_sampler, p);
            near = max(near, max(max(keys.x, keys.y), max(keys.z, keys.w)));
            far = min(far, min(min(keys.x, keys.y), min(keys.z, keys.w)));
        }
    }
    return vec4f(max(coc_of(near), coc_of(far)), 0.0, 0.0, 1.0);
}

// The largest blur in the tiles around each tile. This is how far the pixels of the tile
// gather.
@fragment
fn fs_near(in: FullscreenOut) -> @location(0) vec4f {
    let t = vec2i(in.clip.xy);
    let tile = i32(post.dof.w);
    let tsize = vec2i(textureDimensions(tiles));
    let reach = i32(ceil(post.dof.z / f32(tile)));
    var m = 0.0;
    for (var dy = -reach; dy <= reach; dy++) {
        for (var dx = -reach; dx <= reach; dx++) {
            let q = clamp(t + vec2i(dx, dy), vec2i(0), tsize - 1);
            m = max(m, textureLoad(tiles, q, 0).r);
        }
    }
    return vec4f(m, 0.0, 0.0, 1.0);
}

// Samples farther than this many pixels from their pixel come from the half resolution
// frame. The blur is wide there, and the cache holds more of the smaller frame.
const DOF_INNER: f32 = 6.0;

struct HalfOut {
    @location(0) color: vec4f,
    @location(1) key: vec4f,
}

// The frame and its depth keys, averaged over 2x2 pixels.
@fragment
fn fs_half(in: FullscreenOut) -> HalfOut {
    let origin = vec2i(in.clip.xy) * 2;
    let size = vec2i(textureDimensions(src));
    var color = vec4f(0.0);
    var key = 0.0;
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            let p = min(origin + vec2i(x, y), size - 1);
            color += textureLoad(src, p, 0);
            key += textureLoad(depth_key, p, 0).r;
        }
    }
    var out: HalfOut;
    out.color = color * 0.25;
    out.key = vec4f(key * 0.25, 0.0, 0.0, 1.0);
    return out;
}

// Gathering depth of field. The samples lie on a golden angle spiral, out to the largest
// blur nearby. A sample counts when its own blur reaches this pixel. Samples behind a sharp
// pixel blur only a little into it.
@fragment
fn fs_dof(in: FullscreenOut) -> @location(0) vec4f {
    let p = vec2i(in.clip.xy);
    let size = vec2i(textureDimensions(src));
    let tile = i32(post.dof.w);
    // Here `tiles` holds the output of `fs_near`.
    let radius_max = textureLoad(tiles, p / tile, 0).r;
    let center = textureLoad(src, p, 0);
    if (radius_max < 0.5) {
        return center;
    }
    let center_key = textureLoad(depth_key, p, 0).r;
    let center_coc = coc_of(center_key);
    var sum = center;
    var count = 1.0;
    var radius = 0.8;
    var angle = 0.0;
    // The inner samples come from the full frame. Each source has its own loop, because a
    // branch in one loop makes the GPU fetch from both sources.
    loop {
        if (radius >= min(radius_max, DOF_INNER)) {
            break;
        }
        // A sample past the edge takes the edge pixel.
        let s = clamp(vec2i(round(vec2f(p) + vec2f(cos(angle), sin(angle)) * radius)), vec2i(0), size - 1);
        let key = textureLoad(depth_key, s, 0).r;
        var sample_coc = coc_of(key);
        // The sample is farther away than the center (it has a smaller key).
        if (key < center_key) {
            sample_coc = min(sample_coc, center_coc * 2.0);
        }
        let m = clamp(sample_coc - radius + 0.5, 0.0, 1.0);
        sum += mix(sum / count, textureLoad(src, s, 0), m);
        count += 1.0;
        radius += 1.6 / radius;
        angle += 2.399963;
    }
    // The outer samples come from the half resolution frame, at half the density. They are
    // 2x2 averages already.
    let half_size = vec2i(textureDimensions(aux_tex));
    loop {
        if (radius >= radius_max) {
            break;
        }
        // The 2x2 block that contains the sample point.
        let offset = vec2f(cos(angle), sin(angle)) * radius;
        let s = clamp(vec2i(floor((vec2f(p) + 0.5 + offset) * 0.5)), vec2i(0), half_size - 1);
        let key = textureLoad(half_key, s, 0).r;
        var sample_coc = coc_of(key);
        if (key < center_key) {
            sample_coc = min(sample_coc, center_coc * 2.0);
        }
        let m = clamp(sample_coc - radius + 0.5, 0.0, 1.0);
        sum += mix(sum / count, textureLoad(aux_tex, s, 0), m);
        count += 1.0;
        radius += 3.2 / radius;
        angle += 2.399963;
    }
    return sum / count;
}

// The ambient occlusion (`ambient_occlusion` in `render/post.rs`).

const AO_SAMPLES: i32 = 12;
const AO_BIAS: f32 = 0.25;

// The view space position of a pixel (x right, y down, z forward), w 0 where there is no
// surface.
fn ao_position(q: vec2i) -> vec4f {
    let size = vec2i(textureDimensions(depth_key));
    let p = clamp(q, vec2i(0), size - 1);
    let key = textureLoad(depth_key, p, 0).r;
    if (key <= 0.0) {
        return vec4f(0.0);
    }
    let z = 1.0 / key;
    let xy = (vec2f(p) + 0.5 - vec2f(size) * 0.5) * (z / post.ao.x);
    return vec4f(xy, z, 1.0);
}

// The step to the neighbor nearer in depth, `a` before and `b` after the center.
fn ao_side(c: vec3f, a: vec4f, b: vec4f) -> vec3f {
    if (a.w > 0.0 && b.w > 0.0) {
        if (abs(c.z - a.z) < abs(b.z - c.z)) {
            return c - a.xyz;
        }
        return b.xyz - c;
    }
    if (a.w > 0.0) {
        return c - a.xyz;
    }
    if (b.w > 0.0) {
        return b.xyz - c;
    }
    return vec3f(0.0);
}

// The unblurred occlusion of a pixel, 1 is open.
@fragment
fn fs_ao(in: FullscreenOut) -> @location(0) vec4f {
    let p = vec2i(in.clip.xy);
    let center = ao_position(p);
    if (center.w <= 0.0) {
        return vec4f(1.0, 0.0, 0.0, 1.0);
    }
    let c = center.xyz;
    let dx = ao_side(c, ao_position(p - vec2i(2, 0)), ao_position(p + vec2i(2, 0)));
    let dy = ao_side(c, ao_position(p - vec2i(0, 2)), ao_position(p + vec2i(0, 2)));
    let cr = cross(dy, dx);
    if (dot(cr, cr) <= 0.0) {
        return vec4f(1.0, 0.0, 0.0, 1.0);
    }
    var n = normalize(cr);
    if (dot(n, c) > 0.0) {
        n = -n;
    }
    let radius = min(post.ao.y * post.ao.x / c.z, post.ao.w);
    if (radius < 1.0) {
        return vec4f(1.0, 0.0, 0.0, 1.0);
    }
    let r2 = post.ao.y * post.ao.y;
    let angle = bayer(p) * 6.2831853;
    var sum = 0.0;
    for (var i = 0; i < AO_SAMPLES; i++) {
        let t = (f32(i) + 0.5) / f32(AO_SAMPLES);
        let a = angle + f32(i) * 2.399963;
        let offset = vec2f(cos(a), sin(a)) * (radius * t);
        let q = ao_position(p + vec2i(round(offset)));
        if (q.w <= 0.0) {
            continue;
        }
        let v = q.xyz - c;
        let vv = dot(v, v);
        if (vv < 1e-12) {
            continue;
        }
        let cosine = dot(v, n) / sqrt(vv);
        sum += max(cosine - AO_BIAS, 0.0) * max(1.0 - vv / r2, 0.0);
    }
    let ao = clamp(1.0 - post.ao.z * 2.5 * sum / f32(AO_SAMPLES), 0.0, 1.0);
    return vec4f(ao, 0.0, 0.0, 1.0);
}

// The occlusion blurred over 4x4 pixels of similar depth, as a factor for the frame. The
// pipeline multiplies the frame by it. Here `tiles` holds the output of `fs_ao`.
@fragment
fn fs_ao_apply(in: FullscreenOut) -> @location(0) vec4f {
    let p = vec2i(in.clip.xy);
    let size = vec2i(textureDimensions(depth_key));
    let key = textureLoad(depth_key, p, 0).r;
    if (key <= 0.0) {
        return vec4f(1.0);
    }
    var sum = 0.0;
    var count = 0.0;
    for (var dy = -2; dy < 2; dy++) {
        for (var dx = -2; dx < 2; dx++) {
            let q = clamp(p + vec2i(dx, dy), vec2i(0), size - 1);
            let k = textureLoad(depth_key, q, 0).r;
            if (abs(k - key) <= 0.1 * key) {
                sum += textureLoad(tiles, q, 0).r;
                count += 1.0;
            }
        }
    }
    let ao = sum / max(count, 1.0);
    return vec4f(ao, ao, ao, 1.0);
}

// Adds a weighted moment to the motion blur average. The pipeline blends additively.
@fragment
fn fs_accumulate(in: FullscreenOut) -> @location(0) vec4f {
    return textureLoad(src, vec2i(in.clip.xy), 0) * post.encode.w;
}

// The bloom.

// The bright parts of the frame at half size, with a soft knee at the threshold.
@fragment
fn fs_bright(in: FullscreenOut) -> @location(0) vec4f {
    let d = post.texel.xy;
    let c = (textureSampleLevel(src, src_sampler, in.uv + vec2f(-0.5, -0.5) * d, 0.0)
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(0.5, -0.5) * d, 0.0)
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(-0.5, 0.5) * d, 0.0)
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(0.5, 0.5) * d, 0.0)).rgb * 0.25;
    let brightness = max(c.r, max(c.g, c.b));
    let threshold = post.bloom.x;
    let knee = post.bloom.y;
    var soft = clamp(brightness - threshold + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee);
    let contribution = max(soft, brightness - threshold) / max(brightness, 1e-4);
    return vec4f(c * max(contribution, 0.0), 1.0);
}

// The source at half size: the average of four bilinear samples.
fn down(uv: vec2f) -> vec4f {
    let d = post.texel.xy;
    let c = textureSampleLevel(src, src_sampler, uv + vec2f(-d.x, -d.y), 0.0)
        + textureSampleLevel(src, src_sampler, uv + vec2f(d.x, -d.y), 0.0)
        + textureSampleLevel(src, src_sampler, uv + vec2f(-d.x, d.y), 0.0)
        + textureSampleLevel(src, src_sampler, uv + vec2f(d.x, d.y), 0.0);
    return c * 0.25;
}

@fragment
fn fs_down(in: FullscreenOut) -> @location(0) vec4f {
    return vec4f(down(in.uv).rgb, 1.0);
}

// `fs_down` that keeps the coverage, for the levels of the reflection.
@fragment
fn fs_down_rgba(in: FullscreenOut) -> @location(0) vec4f {
    return down(in.uv);
}

// The next smaller level through a 3x3 tent filter, times the level weight. The pipeline
// blends additively.
@fragment
fn fs_up(in: FullscreenOut) -> @location(0) vec4f {
    let d = post.texel.xy;
    var sum = textureSampleLevel(src, src_sampler, in.uv, 0.0).rgb * 4.0;
    sum = sum + (textureSampleLevel(src, src_sampler, in.uv + vec2f(-d.x, 0.0), 0.0).rgb
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(d.x, 0.0), 0.0).rgb
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(0.0, -d.y), 0.0).rgb
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(0.0, d.y), 0.0).rgb) * 2.0;
    sum = sum + textureSampleLevel(src, src_sampler, in.uv + vec2f(-d.x, -d.y), 0.0).rgb
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(d.x, -d.y), 0.0).rgb
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(-d.x, d.y), 0.0).rgb
        + textureSampleLevel(src, src_sampler, in.uv + vec2f(d.x, d.y), 0.0).rgb;
    return vec4f(sum * (post.bloom.z / 16.0), 0.0);
}

// The encoding.

// ACES filmic tone mapping, as in three.js.
fn aces(color: vec3f) -> vec3f {
    let input = mat3x3f(
        vec3f(0.59719, 0.07600, 0.02840),
        vec3f(0.35458, 0.90834, 0.13383),
        vec3f(0.04823, 0.01566, 0.83777),
    );
    let output = mat3x3f(
        vec3f(1.60475, -0.10208, -0.00327),
        vec3f(-0.53108, 1.10813, -0.07276),
        vec3f(-0.07367, -0.00605, 1.07602),
    );
    let v = input * (color / 0.6);
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.432951) + 0.238081;
    return clamp(output * (a / b), vec3f(0.0), vec3f(1.0));
}

// Khronos PBR Neutral tone mapping.
fn neutral(color_in: vec3f) -> vec3f {
    let start = 0.8 - 0.04;
    let desaturation = 0.15;
    var color = color_in;
    let x = min(color.r, min(color.g, color.b));
    var offset = 0.04;
    if (x < 0.08) {
        offset = x - 6.25 * x * x;
    }
    color -= offset;
    let peak = max(color.r, max(color.g, color.b));
    if (peak < start) {
        return max(color, vec3f(0.0));
    }
    let d = 1.0 - start;
    let new_peak = 1.0 - d * d / (peak + d - start);
    color *= new_peak / peak;
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return max(mix(color, vec3f(new_peak), g), vec3f(0.0));
}

// AgX as in three.js (`agx` in `render/post.rs`).
fn agx(color: vec3f) -> vec3f {
    let srgb_to_2020 = mat3x3f(
        vec3f(0.6274, 0.0691, 0.0164),
        vec3f(0.3293, 0.9195, 0.0880),
        vec3f(0.0433, 0.0113, 0.8956),
    );
    let rec2020_to_srgb = mat3x3f(
        vec3f(1.6605, -0.1246, -0.0182),
        vec3f(-0.5876, 1.1329, -0.1006),
        vec3f(-0.0728, -0.0083, 1.1187),
    );
    let inset = mat3x3f(
        vec3f(0.85662715, 0.13731897, 0.11189821),
        vec3f(0.09512124, 0.76124199, 0.07679942),
        vec3f(0.04825161, 0.10143904, 0.81130237),
    );
    let outset = mat3x3f(
        vec3f(1.1271006, -0.14132976, -0.14132976),
        vec3f(-0.11060664, 1.1578237, -0.11060664),
        vec3f(-0.01649394, -0.01649394, 1.2519364),
    );
    let min_ev = -12.47393;
    let max_ev = 4.026069;
    let c = max(inset * (srgb_to_2020 * color), vec3f(1e-10));
    let x = clamp((log2(c) - min_ev) / (max_ev - min_ev), vec3f(0.0), vec3f(1.0));
    let x2 = x * x;
    let x4 = x2 * x2;
    let curve = 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
    let out = pow(max(outset * curve, vec3f(0.0)), vec3f(2.2));
    return clamp(rec2020_to_srgb * out, vec3f(0.0), vec3f(1.0));
}

fn to_srgb(c: f32) -> f32 {
    if (c <= 0.0031308) {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

// The 4x4 Bayer matrix ([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]).
// The shader computes it, because a table in a function is built again for each pixel.
fn bayer(p: vec2i) -> f32 {
    let x = p.x & 3;
    let y = p.y & 3;
    let fine = 2 * ((x & 1) ^ (y & 1)) + (y & 1);
    let coarse = 2 * (((x >> 1) & 1) ^ ((y >> 1) & 1)) + ((y >> 1) & 1);
    return (f32(4 * fine + coarse) + 0.5) / 16.0;
}

// Adds the bloom, applies the exposure and the tone mapping, and encodes premultiplied sRGB
// with ordered dithering.
@fragment
fn fs_encode(in: FullscreenOut) -> @location(0) vec4f {
    let p = vec2i(in.clip.xy);
    var px = textureLoad(src, p, 0);
    if (post.encode.z > 0.5) {
        let glow = textureSampleLevel(aux_tex, src_sampler, in.uv, 0.0).rgb;
        if (post.bloom.x > 0.5) {
            px = vec4f(mix(px.rgb, glow * post.bloom.y, min(post.bloom.w, 1.0)), px.a);
        } else {
            px = vec4f(px.rgb + glow * post.bloom.w, px.a);
        }
    }
    let rgb = max(px.rgb, vec3f(0.0)) * post.encode.x;
    // Light without coverage (glows, additive surfaces) becomes partly opaque.
    let alpha = clamp(max(px.a, min(max(rgb.r, max(rgb.g, rgb.b)), 1.0)), 0.0, 1.0);
    if (alpha <= 0.0) {
        return vec4f(0.0);
    }
    let straight = rgb / alpha;
    var mapped = clamp(straight, vec3f(0.0), vec3f(1.0));
    let tone = i32(post.encode.y);
    if (tone == 1) {
        mapped = aces(straight);
    } else if (tone == 2) {
        mapped = clamp(neutral(straight), vec3f(0.0), vec3f(1.0));
    } else if (tone == 3) {
        mapped = agx(straight);
    }
    let a8 = floor(alpha * 255.0 + 0.5) / 255.0;
    let dither = (bayer(p) - 0.5) / 255.0;
    let encoded = vec3f(to_srgb(mapped.r), to_srgb(mapped.g), to_srgb(mapped.b));
    return vec4f(clamp((encoded + dither) * a8, vec3f(0.0), vec3f(1.0)), a8);
}
