// The light that scatters in the air. It follows `render/atmosphere.rs` function by function.
// This file continues `scene.wgsl` in the same module.

// The depth keys of the frame.
@group(1) @binding(0) var haze_key: texture_2d<f32>;
// The depth keys of the half resolution haze.
@group(1) @binding(1) var haze_half_key: texture_2d<f32>;
// The half resolution haze: the added light, and the part of the light that gets through.
@group(1) @binding(2) var haze_tex: texture_2d<f32>;
// `atmosphere::NOISE`.
@group(1) @binding(3) var noise_tex: texture_3d<f32>;
@group(1) @binding(4) var noise_sampler: sampler;

// The haze sees each lamp as a ball of this radius in meters.
const LAMP_RADIUS: f32 = 0.2;

struct HazeOut {
    @location(0) haze: vec4f,
    @location(1) key: vec4f,
}

// Marches the ray of the frame pixel at twice the coordinates of the half resolution pixel.
@fragment
fn fs_haze(in: FullscreenOut) -> HazeOut {
    let q = vec2u(in.clip.xy);
    let size = vec2u(textureDimensions(haze_key));
    let p = q * 2u;
    let key = textureLoad(haze_key, p, 0).r;
    let ndc = vec2f(
        (f32(p.x) + 0.5) / f32(size.x) * 2.0 - 1.0,
        1.0 - (f32(p.y) + 0.5) / f32(size.y) * 2.0,
    );
    let near = frame.inv_view_proj * vec4f(ndc, 1.0, 1.0);
    let far = frame.inv_view_proj * vec4f(ndc, 0.0, 1.0);
    let ray = normalize_or(far.xyz / far.w - near.xyz / near.w, frame.forward.xyz);
    var out: HazeOut;
    out.haze = march(ray, key, jitter(q));
    out.key = vec4f(key, 0.0, 0.0, 1.0);
    return out;
}

// The light that the air adds along `ray` up to the surface at depth key `key`, and the part
// of the light of the surface that gets through (in `w`).
fn march(ray: vec3f, key: f32, jitter: f32) -> vec4f {
    // A directional light scatters the same part of its light at each step. Only its shadow
    // changes.
    var constant = frame.haze_ambient.rgb;
    var sun0 = vec3f(0.0);
    var sun1 = vec3f(0.0);
    for (var i = 0; i < i32(frame.counts.x); i++) {
        let l = frame.lights[i];
        if (i32(l.color.w) != 0) {
            continue;
        }
        let light = l.color.rgb * phase(dot(l.direction.xyz, -ray), frame.haze_ambient.w);
        let index = i32(l.params.z);
        if (index == 0) {
            sun0 += light;
        } else if (index == 1) {
            sun1 += light;
        } else {
            constant += light;
        }
    }

    var end = frame.haze_shape.w;
    if (key > 0.0) {
        end = min(end, 1.0 / (key * max(dot(ray, frame.forward.xyz), 1e-4)));
    }
    let steps = u32(frame.counts.w);
    let n = f32(steps);
    var scatter = vec3f(0.0);
    var through = 1.0;
    for (var s = 0u; s < steps; s++) {
        let i = f32(s);
        let u = (i + jitter) / n;
        let t = end * u * u;
        let dt = end * (2.0 * i + 1.0) / (n * n);
        let p = frame.camera.xyz + ray * t;
        let density = haze_density(p);
        if (density <= 0.0) {
            continue;
        }
        var light = constant + haze_lamps(p, ray);
        if (any(sun0 > vec3f(0.0))) {
            light += sun0 * shadow_lit(0, p);
        }
        if (any(sun1 > vec3f(0.0))) {
            light += sun1 * shadow_lit(1, p);
        }
        let absorbed = exp(-density * dt);
        scatter += light * (through * (1.0 - absorbed));
        through *= absorbed;
    }
    return vec4f(scatter * frame.haze.rgb, through);
}

fn haze_density(p: vec3f) -> f32 {
    let shape = frame.haze_shape;
    var density = frame.haze.w * exp(-shape.y * max(p.y - shape.x, 0.0));
    if (shape.z > 0.0) {
        let noise = frame.haze_noise;
        density *= 1.0 + shape.z * (2.0 * fractal_noise(p * noise.w + noise.xyz) - 1.0);
    }
    return density;
}

// The light of the point and spot lights that the air at `p` scatters back along `ray`.
fn haze_lamps(p: vec3f, ray: vec3f) -> vec3f {
    var light = vec3f(0.0);
    for (var i = 0; i < i32(frame.counts.x); i++) {
        let l = frame.lights[i];
        let kind = i32(l.color.w);
        if (kind == 0) {
            continue;
        }
        let d = p - l.position.xyz;
        let dist = max(length(d), LAMP_RADIUS);
        let travel = d / dist;
        var radiance = l.color.rgb * attenuation(dist, l.position.w, l.direction.w);
        if (kind == 2) {
            radiance *= smooth_step(l.params.x, l.params.y, dot(travel, l.direction.xyz));
        }
        light += radiance * phase(dot(travel, -ray), frame.haze_ambient.w);
    }
    return light;
}

// How much light of a shadow casting light reaches `position` in the air. One tap.
fn shadow_lit(index: i32, position: vec3f) -> f32 {
    let s = &frame.shadows[index];
    let clip = s.view_proj * vec4f(position, 1.0);
    let p = clip.xyz / clip.w;
    let uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0 || p.z > 1.0) {
        return 1.0;
    }
    let params = s.params;
    let z = p.z - s.params2.x * params.y;
    let lit = textureSampleCompareLevel(shadow_maps, shadow_sampler, uv, index, z);
    return 1.0 - params.w * (1.0 - lit);
}

// The Henyey-Greenstein phase function.
fn phase(cos: f32, g: f32) -> f32 {
    let x = max(1.0 + g * g - 2.0 * g * cos, 1e-4);
    return (1.0 - g * g) / (4.0 * PI * x * sqrt(x));
}

// A 4x4 Bayer pattern.
const BAYER = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);

fn jitter(q: vec2u) -> f32 {
    return (BAYER[(q.y % 4u) * 4u + q.x % 4u] + 0.5) / 16.0;
}

fn fractal_noise(p: vec3f) -> f32 {
    return (value_noise(p) * 2.0 + value_noise(p * 2.03 + 17.0)) / 3.0;
}

// Value noise with smoothstep weights: the linear filter of the sampler interpolates between
// moved sample positions.
fn value_noise(p: vec3f) -> f32 {
    let cell = floor(p);
    let f = p - cell;
    let q = cell + f * f * (3.0 - 2.0 * f) + 0.5;
    return textureSampleLevel(noise_tex, noise_sampler, q / f32(textureDimensions(noise_tex).x), 0.0).r;
}

// Adds the haze to the frame: a box filter, 4 half resolution samples wide, around the
// pixel. Samples at another depth get less weight. The blend state multiplies the frame by
// `w` and adds `rgb`.
@fragment
fn fs_haze_apply(in: FullscreenOut) -> @location(0) vec4f {
    let p = vec2i(in.clip.xy);
    let own = textureLoad(haze_key, p, 0).r;
    let last = vec2i(textureDimensions(haze_tex)) - 1;
    let base = p / 2;
    let f = vec2f(p % 2) * 0.5;
    var sum = vec4f(0.0);
    var total = 0.0;
    for (var j = -2; j <= 2; j++) {
        let wy = cover(j, f.y);
        for (var i = -2; i <= 2; i++) {
            var w = cover(i, f.x) * wy;
            if (w == 0.0) {
                continue;
            }
            let q = clamp(base + vec2i(i, j), vec2i(0), last);
            let k = textureLoad(haze_half_key, q, 0).r;
            let difference = abs(k - own) / max(max(k, own), 1e-6);
            w /= 0.02 + difference;
            sum += textureLoad(haze_tex, q, 0) * w;
            total += w;
        }
    }
    return sum / total;
}

// The part of the sample at offset `i` that the box covers.
fn cover(i: i32, f: f32) -> f32 {
    return clamp(2.5 - abs(f32(i) - f), 0.0, 1.0);
}
