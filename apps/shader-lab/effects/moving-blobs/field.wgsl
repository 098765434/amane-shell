// pass 0 of moving blobs: the liquid field, its value and its slope, which
// the composite reads to bend, tint and shade the drawing; see effect.json
//
// values: row 0 scale % (lane 3), row 1 detail % (lane 0), row 2 speed % (lane 1)
//
// changed from the Figma original: it's worked out at full size, not half,
// as amane's passes are all the canvas's size; its time is amane's

@fragment
fn main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let frequency = 3.33 * 100.0 / clamp(values[0].w, 30.0, 300.0);

    let speed = clamp(values[2].y, 0.0, 100.0);
    let loop_seconds = 90.0 - speed / 100.0 * 78.0;

    // the loop turns once a loop, so it starts again seamlessly
    var phase = 0.45;

    if (speed > 0.0) {
        phase = fract(time / loop_seconds + 0.45);
    }

    let angle = phase * 6.28318530718;
    let loop_a = vec2<f32>(cos(angle), sin(angle));
    let loop_b = vec2<f32>(-loop_a.y, loop_a.x);

    let core = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
    let p = (core - 0.5) * vec2<f32>(size.x / size.y, 1.0);

    let n = liquid(p, loop_a, loop_b, frequency);

    // a fixed step, so the slope is smooth from pixel to pixel, as the original says
    let e = 0.008;
    let slope = vec2<f32>(
        liquid(p + vec2<f32>(e, 0.0), loop_a, loop_b, frequency) - n,
        liquid(p + vec2<f32>(0.0, e), loop_a, loop_b, frequency) - n,
    ) / e;

    return vec4<f32>(n, slope, 1.0);
}

fn liquid(p: vec2<f32>, loop_a: vec2<f32>, loop_b: vec2<f32>, frequency: f32) -> f32 {
    let detail = clamp(values[1].x, 0.0, 100.0) / 100.0 * 0.45;

    let warp_x = noise4(vec4<f32>(p * frequency * 0.58 + vec2<f32>(5.3, 1.7), loop_b * 0.72));
    let warp_y = noise4(vec4<f32>(p * frequency * 0.58 + vec2<f32>(-2.1, 8.4), loop_a * 0.72));

    let warped = p + (vec2<f32>(warp_x, warp_y) - 0.5) * 0.52;
    let broad = noise4(vec4<f32>(warped * frequency, loop_a * 0.88));

    // with no detail, a fourth 4D noise is skipped
    if (detail <= 0.0001) {
        return broad;
    }

    let fine = noise4(vec4<f32>(warped * frequency * 1.92 + 11.0, loop_b * 1.18));

    return mix(broad, fine, detail);
}

fn hash_bits(p: vec4<i32>) -> u32 {
    var h = bitcast<u32>(p.x) * 0x8da6b343u;
    h = h ^ (bitcast<u32>(p.y) * 0xd8163841u);
    h = h ^ (bitcast<u32>(p.z) * 0xcb1ab31fu);
    h = h ^ (bitcast<u32>(p.w) * 0x165667b1u);
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;

    return h ^ (h >> 16u);
}

fn gradient4(cell: vec4<i32>) -> vec4<f32> {
    let h = hash_bits(cell);
    let channels = vec4<u32>(h & 255u, (h >> 8u) & 255u, (h >> 16u) & 255u, (h >> 24u) & 255u);
    let gradient = vec4<f32>(channels) * (2.0 / 255.0) - 1.0;

    return normalize(gradient + vec4<f32>(0.0001));
}

fn corner(cell: vec4<i32>, offset: vec4<f32>) -> f32 {
    let falloff = 0.6 - dot(offset, offset);

    if (falloff <= 0.0) {
        return 0.0;
    }

    let falloff2 = falloff * falloff;

    return falloff2 * falloff2 * dot(gradient4(cell), offset);
}

// 4D simplex noise from 0 to 1
fn noise4(p: vec4<f32>) -> f32 {
    let f4 = 0.30901699437494745;
    let g4 = 0.1381966011250105;

    let cell = vec4<i32>(floor(p + dot(p, vec4<f32>(f4))));
    let cellf = vec4<f32>(cell);
    let x0 = p - cellf + dot(cellf, vec4<f32>(g4));

    var rank = vec4<i32>(0);

    if (x0.x > x0.y) { rank.x += 1; } else { rank.y += 1; }
    if (x0.x > x0.z) { rank.x += 1; } else { rank.z += 1; }
    if (x0.x > x0.w) { rank.x += 1; } else { rank.w += 1; }
    if (x0.y > x0.z) { rank.y += 1; } else { rank.z += 1; }
    if (x0.y > x0.w) { rank.y += 1; } else { rank.w += 1; }
    if (x0.z > x0.w) { rank.z += 1; } else { rank.w += 1; }

    let i1 = vec4<i32>(select(vec4<i32>(0), vec4<i32>(1), rank >= vec4<i32>(3)));
    let i2 = vec4<i32>(select(vec4<i32>(0), vec4<i32>(1), rank >= vec4<i32>(2)));
    let i3 = vec4<i32>(select(vec4<i32>(0), vec4<i32>(1), rank >= vec4<i32>(1)));

    let x1 = x0 - vec4<f32>(i1) + vec4<f32>(g4);
    let x2 = x0 - vec4<f32>(i2) + vec4<f32>(2.0 * g4);
    let x3 = x0 - vec4<f32>(i3) + vec4<f32>(3.0 * g4);
    let x4 = x0 - 1.0 + vec4<f32>(4.0 * g4);

    let sum = corner(cell, x0)
        + corner(cell + i1, x1)
        + corner(cell + i2, x2)
        + corner(cell + i3, x3)
        + corner(cell + vec4<i32>(1), x4);

    return saturate(0.5 + 13.5 * sum);
}
