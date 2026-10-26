// redraws the drawing as lines between two colours, each line as thick as
// the drawing is bright there; ported from the Figma shader "Hatching"
//
// values (see effect.json):
//   row 0   pattern (0 circles, 1 waves, 2 zigzag), density, softness %, frequency
//   row 1   amplitude %, offset across %, offset along %, line width % of the width
//   row 2   angle in degrees, centre x %, centre y %, unused
//   row 3   background colour
//   row 4   foreground colour
//
// changed from the original: Figma's renderScale is amane's scale, so the
// 4 by 4 samples still fall on real pixels; it returns plain alpha

const TAU: f32 = 6.28318530718;

@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) canvas_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let source = textureSampleLevel(input0, input_sampler, canvas_uv, 0.0);

    let place = uv * size;
    let center = values[2].yz / 100.0 * size;

    let density = values[0].y;
    let softness = values[0].z / 100.0;

    let alpha_in = source.a;
    let straight = source.rgb / max(alpha_in, 0.0001);
    let light = saturate(dot(straight, vec3<f32>(0.2126, 0.7152, 0.0722)));
    let thickness = light * density;

    // 4 by 4 looks inside the real pixel, so thin lines don't stair-step
    var coverage = 0.0;

    for (var i = 0; i < 4; i++) {
        for (var j = 0; j < 4; j++) {
            let off = (vec2<f32>(f32(i), f32(j)) + 0.5) / 4.0 - 0.5;
            let height = pattern_height(place + off / scale, center);

            coverage += smoothstep_down(thickness, thickness - softness - 0.001, height);
        }
    }

    coverage /= 16.0;

    let background = vec4<f32>(values[3].rgb * values[3].a, values[3].a);
    let foreground = vec4<f32>(values[4].rgb * values[4].a, values[4].a);

    let result = mix(background, foreground, coverage) * max(alpha_in, 0.0);

    return vec4<f32>(result.rgb / max(result.a, 0.0001), result.a);
}

fn turn(angle: f32) -> mat2x2<f32> {
    let c = cos(angle);
    let s = sin(angle);

    return mat2x2<f32>(c, -s, s, c);
}

fn wave(cycles: f32, amplitude: f32) -> f32 {
    return sin(cycles * TAU) * amplitude;
}

fn zigzag(cycles: f32, amplitude: f32) -> f32 {
    return (abs(fract(cycles - 0.25) * 2.0 - 1.0) - 0.5) * 2.0 * amplitude;
}

// around a circle, a frequency that isn't whole squeezes its last part so the seam closes
fn seamless_cycles(t: f32, frequency: f32) -> f32 {
    let whole = floor(frequency);

    if (fract(frequency) < 0.0001) {
        return t * whole;
    }

    let cutoff = whole / frequency;

    if (t < cutoff) {
        return t * frequency;
    }

    return whole + (t - cutoff) / max(1.0 - cutoff, 0.0001);
}

fn smoothstep_down(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0));

    return t * t * (3.0 - 2.0 * t);
}

// 0 on a line's middle, 1 halfway between lines
fn pattern_height(at: vec2<f32>, center: vec2<f32>) -> f32 {
    let mode = i32(round(values[0].x));
    let line_width = max(values[1].w / 100.0 * size.x, 1.0);
    let frequency = values[0].w;
    let amplitude = values[1].x / 100.0 * max(size.x, size.y);
    let corner = select(0.0, 1.0, mode == 2);
    let stripe_offset = values[1].y / 100.0 * line_width;
    let phase = values[1].z / 100.0;

    var position = turn(radians(values[2].x)) * (at - center);
    var cycles = position.y / max(max(size.x, size.y), 1.0) * frequency;

    if (mode == 0) {
        let around = fract(atan2(position.y, position.x) / TAU + 1.0);
        cycles = seamless_cycles(around, frequency);
    }

    let offset = mix(wave(cycles + phase, amplitude), zigzag(cycles + phase, amplitude), corner);

    if (mode == 0) {
        position = vec2<f32>(length(position) + offset + stripe_offset, position.y);
    } else {
        position = vec2<f32>(position.x + offset + stripe_offset, position.y);
    }

    return abs(fract(position.x / line_width) - 0.5) * 2.0;
}
