// pass 2 of moving blobs: the finished drawing (input0, premultiplied)
// over a black copy of itself, cast down and to the right; see effect.json
//
// values: row 1 shadow % (lane 3)

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let foreground = textureSampleLevel(input0, input_sampler, canvas_uv, 0.0);

    let strength = saturate(clamp(values[1].w, 0.0, 100.0) / 100.0 * 0.6 / 0.6);
    let direction = normalize(vec2<f32>(0.72, 1.0));

    let source = uv - direction * (0.022 * strength);

    var shadow = 0.0;

    if (strength > 0.0 && all(source >= vec2<f32>(0.0)) && all(source <= vec2<f32>(1.0))) {
        let behind = canvas_uv + (source - uv) * size * scale / placement.canvas.xy;

        shadow = textureSampleLevel(input0, input_sampler, behind, 0.0).a * strength * 0.85;
    }

    // premultiplied foreground over black: the colour stays, the coverage grows
    let alpha = foreground.a + shadow * (1.0 - foreground.a);

    return vec4<f32>(foreground.rgb / max(alpha, 0.00001), alpha);
}
