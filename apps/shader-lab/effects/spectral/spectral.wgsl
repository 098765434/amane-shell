// flowing spectral bands, ported from the WebGL background of the Figma
// Community file "Shader Dash"; drawn inside input0's shape, its alpha only
//
// values (see effect.json):
//   row 0   hue shift in degrees, saturation %, brightness %, speed %
//   row 1   zoom %, warp steps
//
// changed from the original: it's drawn in the rectangle's own uv instead of
// gl_FragCoord (y grows down here, so it's flipped back); the original's
// colour panel only ever handed over three numbers, the first colour's hue,
// the second's saturation + 0.5 and the third's lightness + 0.5, which are
// the sliders here; its time is amane's, so the window draws every frame

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let shape = textureSampleLevel(input0, input_sampler, canvas_uv, 0.0).a;

    let hue_shift = values[0].x / 360.0;
    let saturation = values[0].y / 100.0;
    let brightness = values[0].z / 100.0;
    let t = time * values[0].w / 100.0;
    let zoom = values[1].x / 100.0;
    let steps = i32(clamp(values[1].y, 0.0, 16.0));

    // the original's gl_FragCoord, which grows up from the bottom
    let point = vec2<f32>(uv.x, 1.0 - uv.y) * size;

    var p = (2.0 * point - size) / min(size.x, size.y) * zoom;

    for (var step = 0; step < steps; step++) {
        p = vec2<f32>(
            p.y + cos(p.x + t) - sin(p.y * cos(t * 0.2)),
            p.x - sin(p.y - t) - cos(p.x * sin(t * 0.3)),
        );
    }

    var hsv = rgb_to_hsv(spectral(p.y * 50.0 + 500.0 + sin(t * 0.6)));

    hsv.x = fract(hsv.x + hue_shift);
    hsv.y = saturate(hsv.y * saturation);
    hsv.z = saturate(hsv.z * brightness);

    return vec4<f32>(hsv_to_rgb(hsv), shape);
}

// an approximate colour for light of this wavelength in nanometres, black outside 400 to 700
fn spectral(l: f32) -> vec3<f32> {
    var r = 0.0;
    var g = 0.0;
    var b = 0.0;

    if (l >= 400.0 && l < 410.0) {
        let t = (l - 400.0) / 10.0;
        r = 0.33 * t - 0.20 * t * t;
    } else if (l >= 410.0 && l < 475.0) {
        let t = (l - 410.0) / 65.0;
        r = 0.14 - 0.13 * t * t;
    } else if (l >= 545.0 && l < 595.0) {
        let t = (l - 545.0) / 50.0;
        r = 1.98 * t - t * t;
    } else if (l >= 595.0 && l < 650.0) {
        let t = (l - 595.0) / 55.0;
        r = 0.98 + 0.06 * t - 0.40 * t * t;
    } else if (l >= 650.0 && l < 700.0) {
        let t = (l - 650.0) / 50.0;
        r = 0.65 - 0.84 * t + 0.20 * t * t;
    }

    if (l >= 415.0 && l < 475.0) {
        let t = (l - 415.0) / 60.0;
        g = 0.80 * t * t;
    } else if (l >= 475.0 && l < 590.0) {
        let t = (l - 475.0) / 115.0;
        g = 0.8 + 0.76 * t - 0.80 * t * t;
    } else if (l >= 585.0 && l < 639.0) {
        let t = (l - 585.0) / 54.0;
        g = 0.82 - 0.80 * t;
    }

    if (l >= 400.0 && l < 475.0) {
        let t = (l - 400.0) / 75.0;
        b = 2.20 * t - 1.50 * t * t;
    } else if (l >= 475.0 && l < 560.0) {
        let t = (l - 475.0) / 85.0;
        b = 0.7 - t + 0.30 * t * t;
    }

    return vec3<f32>(r, g, b);
}

fn hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);

    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;

    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}
