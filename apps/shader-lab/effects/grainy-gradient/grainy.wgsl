// a warped, breathing gradient with film grain and click ripples, ported from
// the three.js shader of the Figma Community file "Gradient Shader Card";
// drawn inside input0's shape, its alpha only
//
// values (see effect.json):
//   row 0       noise intensity, noise scale, noise speed, wave intensity
//   row 1       wave scales 1, 2 and 3, how many ripples follow from row 14
//   row 2       wave speeds 1, 2 and 3, grain intensity
//   row 3       grain blend (0 add, 1 screen, 2 overlay, 3 soft light, 4 lighten),
//               speed %, stop count, unused
//   rows 4-11   the gradient's stop colors, top first
//   rows 12-13  the gradient's stop positions, four to a row
//   rows 14-23  ripples: x and y from 0 to 1 across the shape, from the top left,
//               and seconds since the click
//
// changed from the original: its iResolution was the browser window, so the
// pattern took the window's shape; here it takes the shape's own size; its
// ripples were placed in -1 to 1 but compared in another space, so they grew
// away from the click; here they grow from it; the palette is a gradient
// that can be swapped, with the original's colours as the first preset

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let shape = textureSampleLevel(input0, input_sampler, canvas_uv, 0.0).a;

    let t = time * values[3].y / 100.0;

    // the original's vUv, which grows up from the bottom
    let flat_uv = vec2<f32>(uv.x, 1.0 - uv.y);

    // centred, one unit across the height
    let p = (flat_uv - 0.5) * vec2<f32>(size.x / size.y, 1.0);

    var warped = warp(p, t);

    warped += snoise(vec3<f32>(warped * values[0].y, t * values[0].z)) * values[0].x;

    let phase1 = t * 0.6;
    let phase2 = t * 0.4;

    let arch = 1.0 - length(warped) * 0.5;

    let wave1 = sin(warped.x * 3.0 + phase1) * 0.5 * arch;
    let wave2 = sin(warped.x * 5.0 - phase2) * 0.3 * arch;
    let wave3 = sin(warped.y * 4.0 + phase1 * 0.7) * 0.15;
    let parabola = -warped.x * warped.x * 0.2;

    let breathing = sin(t * 0.5) * 0.1 + 0.9;
    let combined = (wave1 + wave2 + wave3 + parabola) * breathing * 0.3;

    let rings = ripples(p);

    let along = flat_uv.y + combined * 0.3 + rings * 0.2;

    var color = ramp(smoothstep(0.0, 1.0, saturate(1.0 - along)));

    color += vec3<f32>(rings * 0.3);

    return vec4<f32>(grain(color, flat_uv, time), shape);
}

fn warp(p: vec2<f32>, t: f32) -> vec2<f32> {
    let scales = values[1].xyz;
    let speeds = values[2].xyz;

    let n1 = noise(p * scales.x + vec2<f32>(t * speeds.x, 0.0));
    let n2 = noise(p * scales.x + vec2<f32>(0.0, t * speeds.y));
    let n3 = noise(p * scales.y + vec2<f32>(-t * speeds.z, t * speeds.z)) * 0.5;
    let n4 = noise(p * scales.z + vec2<f32>(t * speeds.z, -t * speeds.z)) * 0.3;

    return p + vec2<f32>(n1 + n3, n2 + n4) * values[0].w;
}

// one clean ring from each click, growing one unit a second and gone after two
fn ripples(p: vec2<f32>) -> f32 {
    let count = min(u32(values[1].w), 10u);
    let aspect = size.x / size.y;

    var sum = 0.0;

    for (var index = 0u; index < count; index++) {
        let ripple = values[14u + index];

        let center = vec2<f32>((ripple.x - 0.5) * aspect, 0.5 - ripple.y);
        let age = ripple.z;

        let dist = distance(p, center);
        let radius = age;
        let width = 0.05;

        let ring = smoothstep(radius + width, radius, dist)
            * smoothstep(radius - width, radius, dist);

        let fade = 1.0 - smoothstep(0.0, 2.0, age);
        let wave = sin(dist * 15.0 - age * 8.0) * 0.5 + 0.5;

        sum += ring * fade * wave * 0.3;
    }

    return sum;
}

fn stop_color(index: u32) -> vec3<f32> {
    return values[4u + index].rgb;
}

fn stop_position(index: u32) -> f32 {
    return values[12u + index / 4u][index % 4u];
}

// eased between neighbouring stops, as the original eased between its seven colours
fn ramp(at: f32) -> vec3<f32> {
    let count = u32(values[3].z);

    if (count == 0u) {
        return vec3<f32>(0.0);
    }

    if (count == 1u || at <= stop_position(0u)) {
        return stop_color(0u);
    }

    let last = count - 1u;

    for (var index = 0u; index < last; index++) {
        let next = stop_position(index + 1u);

        if (at <= next) {
            let start = stop_position(index);
            let local = saturate((at - start) / max(next - start, 1e-5));

            return mix(stop_color(index), stop_color(index + 1u), smoothstep(0.0, 1.0, local));
        }
    }

    return stop_color(last);
}

fn grain(color: vec3<f32>, at: vec2<f32>, now: f32) -> vec3<f32> {
    let seed = dot(at, vec2<f32>(12.9898, 78.233));
    let speckle = gaussian(fract(sin(seed) * 43758.5453 + now * 2.0), 0.0, 0.25);

    let amount = values[2].w;
    let speckles = vec3<f32>(speckle) * (1.0 - color);

    switch (u32(values[3].x)) {
        case 0u: {
            return color + speckles * amount;
        }
        case 1u: {
            return mix(color, 1.0 - (1.0 - color) * (1.0 - speckles), amount);
        }
        case 3u: {
            return mix(color, pow(max(color, vec3<f32>(0.0)), pow(vec3<f32>(2.0), 2.0 * (0.5 - speckles))), amount);
        }
        case 4u: {
            return max(color, speckles * amount);
        }
        default: {
            let low = 2.0 * color * speckles;
            let high = 1.0 - 2.0 * (1.0 - color) * (1.0 - speckles);

            return mix(color, select(low, high, color >= vec3<f32>(0.5)), amount);
        }
    }
}

fn gaussian(z: f32, mean: f32, spread: f32) -> f32 {
    return (1.0 / (spread * sqrt(2.0 * 3.1415))) * exp(-((z - mean) * (z - mean)) / (2.0 * spread * spread));
}

fn hash(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));

    return -1.0 + 2.0 * fract(sin(q) * 43758.5453123);
}

// gradient noise
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(dot(hash(i), f), dot(hash(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0)), u.x),
        mix(
            dot(hash(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0)),
            dot(hash(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0)),
            u.x,
        ),
        u.y,
    );
}

fn mod289_3(x: vec3<f32>) -> vec3<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn mod289_4(x: vec4<f32>) -> vec4<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn permute(x: vec4<f32>) -> vec4<f32> {
    return mod289_4(((x * 34.0) + 1.0) * x);
}

fn taylor_inv_sqrt(r: vec4<f32>) -> vec4<f32> {
    return 1.79284291400159 - 0.85373472095314 * r;
}

// Ashima Arts' 3D simplex noise, as the original has it
fn snoise(v: vec3<f32>) -> f32 {
    let c = vec2<f32>(1.0 / 6.0, 1.0 / 3.0);
    let d = vec4<f32>(0.0, 0.5, 1.0, 2.0);

    var i = floor(v + dot(v, c.yyy));
    let x0 = v - i + dot(i, c.xxx);

    let g = step(x0.yzx, x0.xyz);
    let l = 1.0 - g;
    let i1 = min(g.xyz, l.zxy);
    let i2 = max(g.xyz, l.zxy);

    let x1 = x0 - i1 + c.xxx;
    let x2 = x0 - i2 + c.yyy;
    let x3 = x0 - d.yyy;

    i = mod289_3(i);

    let p = permute(
        permute(
            permute(i.z + vec4<f32>(0.0, i1.z, i2.z, 1.0)) + i.y + vec4<f32>(0.0, i1.y, i2.y, 1.0),
        ) + i.x + vec4<f32>(0.0, i1.x, i2.x, 1.0),
    );

    let n = 0.142857142857;
    let ns = n * d.wyz - d.xzx;

    let j = p - 49.0 * floor(p * ns.z * ns.z);

    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_);

    let x = x_ * ns.x + ns.yyyy;
    let y = y_ * ns.x + ns.yyyy;
    let h = 1.0 - abs(x) - abs(y);

    let b0 = vec4<f32>(x.xy, y.xy);
    let b1 = vec4<f32>(x.zw, y.zw);

    let s0 = floor(b0) * 2.0 + 1.0;
    let s1 = floor(b1) * 2.0 + 1.0;
    let sh = -step(h, vec4<f32>(0.0));

    let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww;

    var p0 = vec3<f32>(a0.xy, h.x);
    var p1 = vec3<f32>(a0.zw, h.y);
    var p2 = vec3<f32>(a1.xy, h.z);
    var p3 = vec3<f32>(a1.zw, h.w);

    let norm = taylor_inv_sqrt(vec4<f32>(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));

    p0 *= norm.x;
    p1 *= norm.y;
    p2 *= norm.z;
    p3 *= norm.w;

    var m = max(0.6 - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), vec4<f32>(0.0));

    m = m * m;

    return 42.0 * dot(m * m, vec4<f32>(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}
