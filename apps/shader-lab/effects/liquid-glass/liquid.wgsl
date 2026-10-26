// glass that bends what's behind it: inside a bezel along the edge, the
// backdrop (input0) is read from further along the edge's normal, so lines
// behind it curve as they cross the rim, each colour a little apart; the
// surface's tint and glow (input1) lie over it and a rim lit from the top
// left over that, as on the shell's glass (kit/src/glass.wgsl)
//
// values (see effect.json):
//   row 0   refraction in logical pixels (negative bends the other way),
//           bezel width, dispersion, how the bend grows toward the rim
//   row 1   rim strength, rim width, far catch, bloom
//   row 2   the rim's colour
//   row 3   the surface's rectangle: x, y, width, height (the lab fills it)
//   row 4   the surface's corner radius (the lab fills it)
//
// the shell can't do this yet: a surface can't read what's behind it, so
// lensing over other windows has to be drawn by niri

// distance to the rounded rectangle's edge, below 0 inside
fn rounded(point: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let q = abs(point) - half + vec2<f32>(radius);

    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let rect = values[3];
    let half = rect.zw * 0.5;
    let radius = min(values[4].x, min(half.x, half.y));

    let point = uv * size - rect.xy - half;
    let distance = rounded(point, half, radius);

    // one real pixel of softening on the outline
    let pixel = 1.0 / scale;
    let shape = saturate(0.5 - distance / pixel);

    // which way the edge faces, from how the distance changes nearby
    let e = 0.5;
    let normal = normalize(vec2<f32>(
        rounded(point + vec2<f32>(e, 0.0), half, radius) - rounded(point - vec2<f32>(e, 0.0), half, radius),
        rounded(point + vec2<f32>(0.0, e), half, radius) - rounded(point - vec2<f32>(0.0, e), half, radius),
    ) + vec2<f32>(1e-6));

    // 0 deep inside, 1 at the rim
    let bezel = max(values[0].y, 0.001);
    let toward_rim = saturate(1.0 + distance / bezel);
    let bend = pow(toward_rim, max(values[0].w, 0.1)) * values[0].x;

    // a logical pixel on the canvas
    let step = scale / placement.canvas.xy;
    let dispersion = values[0].z;

    let red = backdrop(canvas_uv - normal * bend * (1.0 + dispersion * 0.5) * step).r;
    let green = backdrop(canvas_uv - normal * bend * step).g;
    let blue = backdrop(canvas_uv - normal * bend * (1.0 - dispersion * 0.5) * step).b;

    var color = vec3<f32>(red, green, blue);

    // the tint and glow, premultiplied, over the bent backdrop
    let surface = textureSampleLevel(input1, input_sampler, canvas_uv, 0.0);

    color = surface.rgb + color * (1.0 - surface.a);

    // the rim: brightest facing the top left, a fainter catch on the far edge
    let toward_light = dot(normal, normalize(vec2<f32>(-1.0, -1.0)));
    let near = pow(max(toward_light, 0.0), 2.0);
    let far = pow(max(-toward_light, 0.0), 3.0) * values[1].z;
    let lit = 0.15 + 0.85 * near + far;

    let inside = 1.0 - smoothstep(-pixel, 0.0, distance);
    let line = inside * (1.0 - smoothstep(values[1].y - pixel, values[1].y, -distance)) * values[1].x * lit;
    let bloom = inside * (1.0 - smoothstep(0.0, bezel, -distance)) * values[1].w * (near + far);

    color = mix(color, values[2].rgb, saturate(line + bloom));

    return vec4<f32>(color, shape);
}

fn backdrop(at: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(input0, input_sampler, at, 0.0);
}
