// metallic chrome over the drawing's shape, ported from a Figma layer shader;
// input1 is the drawing's blurred alpha, which is both the bevel and the
// rounded outline, so only the drawing's alpha matters, never its color
//
// values (see effect.json):
//   row 0       x: rounding, the blur in logical pixels (read by blur_x and blur_y)
//   rows 1-8    the gradient's stop colors
//   rows 9-10   the gradient's stop positions, four to a row
//   row 11      stop count, depth %, rgb split %, scale %
//   row 12      phase %, evolution %, angle in degrees, stretch %
//   row 13      offset %, repeats, roughness %
//
// changed from the Figma original: it returns plain alpha, not premultiplied;
// the blur is in logical pixels times scale and reaches 64 pixels, not 32,
// two taps at a time, into half floats; fwidth is taken before the loops,
// so derivative_uniformity stays on; the one-octave fbm is just the noise

// x and z repeat every 8, so phase and evolution loop every 100%
const TILE_PERIOD: f32 = 8.0;

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let depth = values[11].y / 100.0;
    let split = values[11].z / 1000.0;
    let noise_scale = max(0.01, values[11].w / 100.0);
    let phase = values[12].x / 100.0 * TILE_PERIOD;
    let evolution = values[12].y / 100.0 * TILE_PERIOD;
    let angle = radians(values[12].z);
    let stretch = max(0.01, values[12].w / 100.0);
    let offset = values[13].x / 100.0;
    let repeats = max(1.0, floor(values[13].y + 0.5));
    let roughness = max(0.0, values[13].z / 100.0);

    let centered = uv - vec2<f32>(0.5);
    let turned = vec2<f32>(
        centered.x * cos(angle) - centered.y * sin(angle),
        centered.x * sin(angle) + centered.y * cos(angle),
    ) + vec2<f32>(0.5);

    let edge = textureSampleLevel(input1, input_sampler, canvas_uv, 0.0).a;

    let base = vec3<f32>(
        turned.x / noise_scale + phase,
        turned.y / noise_scale / stretch,
        0.37 + evolution,
    );

    let eps = 0.005 / noise_scale;

    // the noise's slope, which the rgb split pushes each channel along
    let slope = vec2<f32>(
        noise(base + vec3<f32>(eps, 0.0, 0.0)) - noise(base - vec3<f32>(eps, 0.0, 0.0)),
        noise(base + vec3<f32>(0.0, eps, 0.0)) - noise(base - vec3<f32>(0.0, eps, 0.0)),
    );

    let center = mix(0.5, noise(base) * edge, depth) + (slope.x + slope.y) * 0.5 * depth * 0.3;

    let shift = vec3<f32>(normalize(slope + vec2<f32>(1e-6)) * split, 0.0);

    var rough = 0.0;

    if (roughness > 0.0) {
        let frequency = 1000.0;
        let fine = smooth_noise(
            vec3<f32>(turned * frequency, 0.37 * frequency),
            TILE_PERIOD * frequency,
        );

        rough = (fine * 2.0 - 1.0) * roughness * 0.05 * edge;
    }

    let height_r = saturate(height(base + shift, depth, edge, eps) + rough);
    let height_g = saturate(center + rough);
    let height_b = saturate(height(base - shift, depth, edge, eps) + rough);

    // how far each height moves across one pixel, worked out before any loop
    let width_r = fwidth(height_r) * repeats;
    let width_g = fwidth(height_g) * repeats;
    let width_b = fwidth(height_b) * repeats;

    let red = smoothed(height_r * repeats, width_r, offset).r;
    let green = smoothed(height_g * repeats, width_g, offset).g;
    let blue = smoothed(height_b * repeats, width_b, offset).b;

    let alpha = smoothstep(0.45, 0.55, edge);

    return vec4<f32>(red, green, blue, alpha);
}

fn height(at: vec3<f32>, depth: f32, edge: f32, eps: f32) -> f32 {
    let x = noise(at + vec3<f32>(eps, 0.0, 0.0)) - noise(at - vec3<f32>(eps, 0.0, 0.0));
    let y = noise(at + vec3<f32>(0.0, eps, 0.0)) - noise(at - vec3<f32>(0.0, eps, 0.0));

    return mix(0.5, noise(at) * edge, depth) + (x + y) * 0.5 * depth * 0.3;
}

// four looks spread across the pixel, so the gradient's hard stops don't stair-step
fn smoothed(level: f32, width: f32, offset: f32) -> vec4<f32> {
    var sum = vec4<f32>(0.0);

    for (var index = 0u; index < 4u; index++) {
        let jitter = (f32(index) + 0.5) / 4.0 - 0.5;

        sum += wrapped(level + jitter * width + offset, width);
    }

    return sum / 4.0;
}

// the gradient repeats, and where it wraps its two ends are blended across the pixel
fn wrapped(at: f32, width: f32) -> vec4<f32> {
    let inside = fract(at);
    let soft = max(width, 1e-5);
    let to_wrap = min(inside, 1.0 - inside);
    let blend = saturate(to_wrap / (soft * 1.5));
    let other = select(inside + 1.0, inside - 1.0, inside > 0.5);

    return mix(ramp(clamp(other, 0.0, 1.0)), ramp(inside), blend);
}

fn stop_color(index: u32) -> vec4<f32> {
    return values[1u + index];
}

fn stop_position(index: u32) -> f32 {
    return values[9u + index / 4u][index % 4u];
}

fn ramp(at: f32) -> vec4<f32> {
    let count = u32(values[11].x);

    if (count == 0u) {
        return vec4<f32>(0.0);
    }

    if (count == 1u || at <= stop_position(0u)) {
        return stop_color(0u);
    }

    let last = count - 1u;

    if (at >= stop_position(last)) {
        return stop_color(last);
    }

    for (var index = 0u; index < last; index++) {
        let next = stop_position(index + 1u);

        if (at <= next) {
            let start = stop_position(index);

            return mix(stop_color(index), stop_color(index + 1u), (at - start) / max(next - start, 1e-5));
        }
    }

    return stop_color(last);
}

fn hash(at: vec3<f32>) -> f32 {
    var q = fract(at * vec3<f32>(0.1031, 0.1030, 0.0973));

    q += dot(q, q.yxz + 33.33);

    return fract((q.x + q.y) * q.z);
}

// one octave of value noise, the original's fbm with its single octave
fn noise(at: vec3<f32>) -> f32 {
    return smooth_noise(at, TILE_PERIOD);
}

// value noise whose x and z repeat every period
fn smooth_noise(at: vec3<f32>, period: f32) -> f32 {
    let cell = floor(at);
    let inside = fract(at);
    let s = inside * inside * (3.0 - 2.0 * inside);

    let x0 = cell.x - floor(cell.x / period) * period;
    let x1 = (cell.x + 1.0) - floor((cell.x + 1.0) / period) * period;
    let z0 = cell.z - floor(cell.z / period) * period;
    let z1 = (cell.z + 1.0) - floor((cell.z + 1.0) / period) * period;
    let y0 = cell.y;
    let y1 = cell.y + 1.0;

    return mix(
        mix(
            mix(hash(vec3<f32>(x0, y0, z0)), hash(vec3<f32>(x1, y0, z0)), s.x),
            mix(hash(vec3<f32>(x0, y1, z0)), hash(vec3<f32>(x1, y1, z0)), s.x),
            s.y,
        ),
        mix(
            mix(hash(vec3<f32>(x0, y0, z1)), hash(vec3<f32>(x1, y0, z1)), s.x),
            mix(hash(vec3<f32>(x0, y1, z1)), hash(vec3<f32>(x1, y1, z1)), s.x),
            s.y,
        ),
        s.z,
    );
}
