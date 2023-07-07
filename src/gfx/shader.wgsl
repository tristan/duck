struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) use_tex: f32,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.uv = model.uv;
    out.use_tex = model.pos.w;
    out.clip_position = camera.view_proj * vec4<f32>(model.pos.xyz, 1.0);
    return out;
}

@vertex
fn subpixel_r_vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.uv = model.uv;
    out.use_tex = model.pos.w;
    out.clip_position = camera.view_proj * vec4<f32>(model.pos.xy, 0.3, 1.0);
    return out;
}

@vertex
fn subpixel_g_vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.uv = model.uv;
    out.use_tex = model.pos.w;
    out.clip_position = camera.view_proj * vec4<f32>(model.pos.xy, 0.2, 1.0);
    return out;
}

@vertex
fn subpixel_b_vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.uv = model.uv;
    out.use_tex = model.pos.w;
    out.clip_position = camera.view_proj * vec4<f32>(model.pos.xy, 0.1, 1.0);
    return out;
}

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@fragment
fn transparent_fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var res = in.color;
    let sample = textureSample(t_diffuse, s_diffuse, in.uv);
    if in.use_tex > 0.0 {
        res *= sample;
    }
    return res;
}

@fragment
fn subpixel_r_fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let color = in.color;
    let mask = textureSample(t_diffuse, s_diffuse, in.uv);
	let alpha = gamma_correct_subpx(color, mask);
	let a = alpha.r;
	let rgb = color.rgb; // * alpha.rgb;
	return vec4<f32>(rgb.r, 0.0, 0.0, a);
}

@fragment
fn subpixel_g_fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let color = in.color;
    let mask = textureSample(t_diffuse, s_diffuse, in.uv);
	let alpha = gamma_correct_subpx(color, mask);
	let a = alpha.g;
	let rgb = color.rgb; // * alpha.rgb;
	return vec4<f32>(0.0, rgb.g, 0.0, a);
}

@fragment
fn subpixel_b_fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	let color = in.color;
    let mask = textureSample(t_diffuse, s_diffuse, in.uv);
	let alpha = gamma_correct_subpx(color, mask);
	let a = alpha.b;
	let rgb = color.rgb; // * alpha.rgb;
	return vec4<f32>(0.0, 0.0, rgb.b, a);
}

fn gamma_correct_subpx(color: vec4<f32>, mask: vec4<f32>) -> vec4<f32> {
	let l = luma(color);
	let inverse_luma = 1.0 - l;
	let gamma = mix(1.0 / 1.2, 1.0 / 2.4, inverse_luma);
	let contrast = mix(0.1, 0.8, inverse_luma);
	return vec4<f32>(
        gamma_correct(l, mask.x * color.a, gamma, contrast),
        gamma_correct(l, mask.y * color.a, gamma, contrast),
        gamma_correct(l, mask.z * color.a, gamma, contrast),
        1.0
    );
}

fn luma(color: vec4<f32>) -> f32 {
	return color.x * 0.25 + color.y * 0.72 + color.z * 0.075;
}

fn gamma_correct(luma: f32, alpha: f32, gamma: f32, contrast: f32) -> f32 {
	let inverse_luma = 1.0 - luma;
	let inverse_alpha = 1.0 - alpha;
	let g = pow(luma * alpha + inverse_luma * inverse_alpha, gamma);
	let a = (g - inverse_luma) / (luma - inverse_luma);
	let a0 = a + ((1.0 - a) * contrast * a);
	return clamp(a0, 0.0, 1.0);
}
