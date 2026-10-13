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
        // A long step crosses many shafts of light. The shadow is the mean of taps spread
        // over the step, so the shafts do not alias into ripples.
        let start = end * (i / n) * (i / n);
        if (any(sun0 > vec3f(0.0))) {
            light += sun0 * step_lit(0, ray, start, dt, jitter);
        }
        if (any(sun1 > vec3f(0.0))) {
            light += sun1 * step_lit(1, ray, start, dt, jitter);
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

// The shadow taps of a step in the haze.
const SHADOW_TAPS: i32 = 4;

// The mean light of a shadow casting light over the step from `start` to `start + dt`.
fn step_lit(index: i32, ray: vec3f, start: f32, dt: f32, jitter: f32) -> f32 {
    var lit = 0.0;
    for (var k = 0; k < SHADOW_TAPS; k++) {
        let s = start + dt * (f32(k) + jitter) / f32(SHADOW_TAPS);
        lit += shadow_lit(index, frame.camera.xyz + ray * s);
    }
    return lit / f32(SHADOW_TAPS);
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

// The start of the steps of a half resolution pixel, `0..1`: interleaved gradient noise,
// moved by the golden ratio for each moment of a motion blurred frame (`jitter` in
// `atmosphere.rs`).
fn jitter(q: vec2u) -> f32 {
    let noise = fract(52.982918 * fract(0.06711056 * f32(q.x) + 0.00583715 * f32(q.y)));
    return fract(noise + 0.618034 * frame.haze_march.x);
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
