// The edge light of every translucent surface (components/glass.rs), lit from
// the top-left: a thin line that is brightest facing the light and fades along
// the sides, a fainter catch on the far edge where light bounces back through
// the glass, and a soft bloom just inside the lit edge.
//
// amane redraws a shader every frame if its source names its clock, so this
// file must never mention it, not even in a comment.
//
// values[0]: corner radius, line thickness, line strength, bloom strength
// values[1]: bloom width, far edge strength
// values[2]: the highlight's red, green and blue, from 0 to 1

// distance to the rounded rectangle's edge, below 0 inside
fn edge(point: vec2<f32>, half: vec2<f32>, radius: f32) -> f32 {
    let q = abs(point) - half + vec2<f32>(radius);

    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let radius = values[0].x;
    let thickness = values[0].y;
    let line_strength = values[0].z;
    let bloom_strength = values[0].w;
    let bloom_width = values[1].x;
    let far_strength = values[1].y;

    let half = size * 0.5;
    let point = uv * size - half;

    let distance = edge(point, half, radius);

    // which way the edge faces, from how the distance changes nearby
    let step = 0.5;
    let facing = normalize(vec2<f32>(
        edge(point + vec2<f32>(step, 0.0), half, radius) - edge(point - vec2<f32>(step, 0.0), half, radius),
        edge(point + vec2<f32>(0.0, step), half, radius) - edge(point - vec2<f32>(0.0, step), half, radius),
    ) + vec2<f32>(1e-6));

    // pixels grow down, so up-left is -x, -y
    let light = normalize(vec2<f32>(-1.0, -1.0));
    let toward = dot(facing, light);

    // a little everywhere, so the side edges don't vanish
    let near = pow(max(toward, 0.0), 2.0);
    let far = pow(max(-toward, 0.0), 3.0) * far_strength;
    let lit = 0.15 + 0.85 * near + far;

    // one real pixel of softening at each side of the line
    let blur = fwidth(distance);
    let inside = 1.0 - smoothstep(-blur, 0.0, distance);
    let within_line = 1.0 - smoothstep(thickness - blur, thickness, -distance);
    let line = inside * within_line * line_strength * lit;

    let bloom = inside * (1.0 - smoothstep(0.0, bloom_width, -distance)) * bloom_strength * (near + far);

    return vec4<f32>(values[2].rgb, clamp(line + bloom, 0.0, 1.0));
}
