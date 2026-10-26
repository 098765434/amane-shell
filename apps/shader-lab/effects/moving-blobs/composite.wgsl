// pass 1 of moving blobs: the drawing (input0) bent, split, tinted and
// shaded where the liquid field (input1) says, written premultiplied for
// the shadow pass; see effect.json
//
// values:
//   row 0   mode (0 refraction, 1 displacement, 2 bulge / pinch, 3 hue shift,
//           4 invert), region (0 whole area, 1 edges only), strength %, scale %
//   row 1   detail %, threshold %, softness %, shadow %
//   row 2   dispersion %, speed %
//
// changed source the original: reads are kept inside the rectangle, as the
// original kept them inside its layer; the grain is one speck per logical
// pixel, Figma's renderScale being amane's scale

struct Settings {
    mode: f32,
    edge_only: f32,
    strength: f32,
    threshold: f32,
    softness: f32,
    shadow: f32,
    dispersion: f32,
    hue: vec2<f32>,
}

fn settings() -> Settings {
    let strength = clamp(values[0].z / 100.0, -1.0, 1.0);
    let hue = strength * 3.14159265359;

    return Settings(
        values[0].x,
        values[0].y,
        strength,
        0.25 + clamp(values[1].y, 0.0, 100.0) / 100.0 * 0.5,
        0.001 + clamp(values[1].z, 0.0, 100.0) / 100.0 * (0.12 - 0.001),
        clamp(values[1].w, 0.0, 100.0) / 100.0 * 0.6,
        clamp(values[2].x, 0.0, 100.0) / 100.0 * 0.024,
        vec2<f32>(cos(hue), sin(hue)),
    );
}

// where a point given across the rectangle is on the canvas, kept inside the rectangle
fn on_canvas(at: vec2<f32>, here: vec2<f32>, here_canvas: vec2<f32>) -> vec2<f32> {
    let pixels = size * scale;
    let half = 0.5 / pixels;
    let inside = clamp(at, half, vec2<f32>(1.0) - half);

    return here_canvas + (inside - here) * pixels / placement.canvas.xy;
}

fn read_input(at: vec2<f32>, here: vec2<f32>, here_canvas: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(input0, input_sampler, on_canvas(at, here, here_canvas), 0.0);
}

fn read_field(at: vec2<f32>, here: vec2<f32>, here_canvas: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(input1, input_sampler, on_canvas(at, here, here_canvas), 0.0);
}

fn hash_pixel(p: vec2<u32>) -> f32 {
    var h = (p.x * 0x8da6b343u) ^ (p.y * 0xd8163841u);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);

    return f32(h) * (1.0 / 4294967295.0);
}

fn rotate_hue(color: vec3<f32>, cosine: f32, sine: f32) -> vec3<f32> {
    let axis = normalize(vec3<f32>(1.0));

    return color * cosine + cross(axis, color) * sine + axis * dot(axis, color) * (1.0 - cosine);
}

// how near the blob's edge (rim), and how much it moves the drawing
fn metrics(n: f32, s: Settings) -> vec2<f32> {
    let blob = smoothstep(s.threshold - s.softness, s.threshold + s.softness, n);
    let rim = exp(-abs(n - s.threshold) / max(s.softness * 0.72, 0.002));
    let band = pow(rim, 0.72);

    return vec2<f32>(rim, mix(blob, band, saturate(s.edge_only)));
}

fn shade(at: vec2<f32>, field: vec4<f32>, rim: f32, mask: f32, here: vec2<f32>, here_canvas: vec2<f32>) -> vec4<f32> {
    let s = settings();
    let gradient = field.gb;
    let magnitude = abs(s.strength);

    var source = at;

    if (s.mode < 0.5) {
        let tangent = normalize(vec2<f32>(-gradient.y, gradient.x) + vec2<f32>(0.0001));
        let optical = gradient * mask + tangent * rim * 0.14;

        source = at + optical * (0.075 * s.strength);
    } else if (s.mode < 1.5) {
        // a negative y moves the drawing up
        source = at + vec2<f32>(1.0, -0.18) * (0.075 * s.strength * mask);
    } else if (s.mode < 2.5) {
        let across = gradient * vec2<f32>(size.x / size.y, 1.0);

        source = at + normalize(across + vec2<f32>(0.0001)) * (0.065 * s.strength * mask);
    }

    let middle = read_input(source, here, here_canvas);

    var alpha = middle.a;
    var color = middle.rgb / max(middle.a, 0.00001);

    // coverage splits too, so the dispersion shows on black see-through art
    if (s.dispersion > 0.000001) {
        let split = gradient * s.dispersion * rim;
        let left = read_input(source - split, here, here_canvas);
        let right = read_input(source + split, here, here_canvas);

        let channel_alpha = vec3<f32>(left.a, middle.a, right.a);

        alpha = max(channel_alpha.r, max(channel_alpha.g, channel_alpha.b));

        let premultiplied = vec3<f32>(left.r, middle.g, right.b) + vec3<f32>(alpha) - channel_alpha;

        color = premultiplied / max(alpha, 0.00001);
    }

    var highlight = 0.0;

    if (s.mode < 0.5 || s.shadow > 0.0) {
        let light = normalize(vec2<f32>(-0.7, -1.0));

        highlight = pow(max(dot(normalize(gradient + vec2<f32>(0.0001)), -light), 0.0), 7.0);
    }

    if (s.mode < 0.5) {
        color = mix(color, vec3<f32>(1.0), rim * highlight * 0.34 * magnitude);
    } else if (s.mode > 2.5 && s.mode < 3.5) {
        color = mix(color, rotate_hue(color, s.hue.x, s.hue.y), mask);
    } else if (s.mode > 3.5 && s.mode < 4.5) {
        color = mix(color, vec3<f32>(1.0) - color, magnitude * mask);
    }

    let tonal = saturate(rim * (1.0 - highlight) * s.shadow);

    color = saturate(mix(color, vec3<f32>(0.0), tonal));

    return vec4<f32>(color * alpha, alpha);
}

fn shaded(at: vec2<f32>, here: vec2<f32>, here_canvas: vec2<f32>) -> vec4<f32> {
    let field = read_field(at, here, here_canvas);
    let m = metrics(field.r, settings());

    return shade(at, field, m.x, m.y, here, here_canvas);
}

@fragment
fn main(
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) canvas_uv: vec2<f32>,
) -> @location(0) vec4<f32> {
    let s = settings();
    let here = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));

    let field = read_field(here, uv, canvas_uv);
    let m = metrics(field.r, s);
    let rim = m.x;
    let mask = m.y;

    let center = shade(here, field, rim, mask, uv, canvas_uv);

    // where the bent drawing changes sharply, eight looks inside the pixel smooth it
    let luma = dot(center.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let contrast = fwidth(luma) + fwidth(center.a);

    let supersample = s.mode < 2.5
        && abs(s.strength) > 0.2
        && max(mask, rim) > 0.01
        && contrast > 0.015;

    var result = center;

    if (supersample) {
        let pixel = 1.0 / (size * scale);

        result = (
            shaded(here + pixel * vec2<f32>(-0.4375, 0.0625), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(-0.3125, -0.3125), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(-0.0625, -0.1875), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(0.1875, -0.4375), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(0.3125, -0.0625), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(0.4375, 0.3125), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(0.0625, 0.4375), uv, canvas_uv)
            + shaded(here + pixel * vec2<f32>(-0.1875, 0.1875), uv, canvas_uv)
        ) * 0.125;
    }

    let grain = (hash_pixel(vec2<u32>(floor(position.xy / scale))) - 0.5) * 0.018;
    let grained = clamp(result.rgb + grain * result.a, vec3<f32>(0.0), vec3<f32>(result.a));

    return vec4<f32>(grained, result.a);
}
