struct Params {
    time: f32,
    width: f32,
    height: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318530;

fn palette(t: f32) -> vec3<f32> {
    let a = vec3<f32>(0.5, 0.5, 0.5);
    let b = vec3<f32>(0.5, 0.5, 0.5);
    let c = vec3<f32>(1.0, 1.0, 1.0);
    let d = vec3<f32>(0.263, 0.416, 0.557);
    return a + b * cos(TAU * (c * t + d));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(output_texture);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }

    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) / vec2<f32>(f32(dims.x), f32(dims.y))) * 2.0 - 1.0;
    let aspect = f32(dims.x) / f32(dims.y);
    let p = vec2<f32>(uv.x * aspect, uv.y);

    var color = vec3<f32>(0.0);
    var q = p;

    for (var i = 0; i < 4; i++) {
        q = fract(q * 1.5) - 0.5;
        let d = length(q) * exp(-length(p));
        let col = palette(length(p) + f32(i) * 0.4 + params.time * 0.4);
        let intensity = abs(sin(d * 8.0 + params.time) / d);
        color += col * intensity * 0.02;
    }

    textureStore(output_texture, gid.xy, vec4<f32>(color, 1.0));
}
