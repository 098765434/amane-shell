// expanding rings of light, cut into diagonal steps, each colour a moment
// behind the last; ported from the three.js header of the Figma Community
// file "Copy Shader Animation Component"; drawn inside input0's shape
//
// values (see effect.json):
//   row 0   speed %, line width, line count, ring spread
//   row 1   step size, colour lag, brightness %, glow only (0 or 1)
//
// changed from the original: its time grew 0.05 a frame, so it ran twice as
// fast at 120 Hz as at 60; here it grows with real time, 100% being the
// original at 60 frames a second; exactly on a ring it divided by zero,
// here the distance has a tiny floor; with glow only, the black is
// transparent instead, so the rings can lie over anything

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let shape = textureSampleLevel(input0, input_sampler, canvas_uv, 0.0).a;

    // the original added 0.05 to its time 60 times a second and took 5% of it
    let t = time * 3.0 * 0.05 * values[0].x / 100.0;

    let line_width = values[0].y;
    let lines = i32(clamp(values[0].z, 1.0, 16.0));
    let spread = values[0].w;
    let step_size = max(values[1].x, 0.001);
    let lag = values[1].y;

    // the original's gl_FragCoord grows up from the bottom
    let point = vec2<f32>(uv.x, 1.0 - uv.y) * size;
    let p = (point * 2.0 - size) / min(size.x, size.y);

    // the distance to the centre, set back by a diagonal staircase
    let offset = length(p) - modulo(p.x + p.y, step_size);

    var color = vec3<f32>(0.0);

    for (var channel = 0; channel < 3; channel++) {
        var light = 0.0;

        for (var line = 0; line < lines; line++) {
            let radius = fract(t - lag * f32(channel) + f32(line) * 0.01) * spread;

            light += line_width * f32(line * line) / max(abs(radius - offset), 1e-4);
        }

        color[channel] = light;
    }

    color *= values[1].z / 100.0;

    // past 1 a colour only clips, so it's held there before being made plain
    color = min(color, vec3<f32>(1.0));

    if (values[1].w < 0.5) {
        return vec4<f32>(color, shape);
    }

    // the brightest channel is how much shows, so black is clear
    let alpha = max(color.r, max(color.g, color.b));

    return vec4<f32>(color / max(alpha, 1e-4), alpha * shape);
}

// glsl's mod, which keeps the sign of the divisor; wgsl's % keeps the dividend's
fn modulo(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}
