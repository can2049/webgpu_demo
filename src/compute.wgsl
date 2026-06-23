// ========================================================================
// Compute Shader: GPU 并行分形图像生成
// ========================================================================
//
// 这个 compute shader 在 GPU 上并行运行，每个线程负责计算一个像素的颜色。
// 16x16 的 workgroup_size 意味着每个工作组处理 256 个像素。
//
// 渲染算法: 基于迭代分形的彩色光晕效果
//   1. 将像素坐标归一化到 [-1, 1] 范围（UV 空间）
//   2. 对坐标进行多次迭代变换（fract 重复 + 指数衰减）
//   3. 每次迭代用余弦调色板生成颜色，叠加 sin 波动光晕
//   4. 颜色随时间参数变化，形成动画效果

// 从 Rust 端通过 uniform buffer 传入的参数
struct Params {
    time: f32,       // 动画时间（秒）
    width: f32,      // 画布宽度（像素）
    height: f32,     // 画布高度（像素）
    _padding: f32,   // 16 字节对齐填充
}

// group(0) binding(0): 只读 uniform buffer，每帧从 CPU 端更新
@group(0) @binding(0) var<uniform> params: Params;

// group(0) binding(1): 可写 storage texture，compute shader 的输出目标
// rgba8unorm 表示每通道 8 位、归一化到 [0, 1] 的 RGBA 纹理
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318530;  // 2π，完整圆周

// 余弦调色板函数 (Inigo Quilez 调色板技术)
//
// 公式: color = a + b * cos(2π * (c*t + d))
//
// 通过调整 a, b, c, d 四个向量可以生成各种渐变色带。
// 参考: https://iquilezles.org/articles/palettes/
//
// 参数 t 通常映射为距离或迭代次数，产生色环效果。
fn palette(t: f32) -> vec3<f32> {
    let a = vec3<f32>(0.5, 0.5, 0.5);   // 偏移（中心亮度）
    let b = vec3<f32>(0.5, 0.5, 0.5);   // 振幅（颜色变化范围）
    let c = vec3<f32>(1.0, 1.0, 1.0);   // 频率（RGB 通道同频）
    let d = vec3<f32>(0.263, 0.416, 0.557); // 相位偏移（决定色调分布）
    return a + b * cos(TAU * (c * t + d));
}

// 每个 GPU 线程的入口函数
// @workgroup_size(16, 16) 表示一个工作组是 16×16 = 256 个线程
// global_invocation_id 是当前线程在整个 dispatch 中的全局坐标 (像素坐标)
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(output_texture);

    // 边界检查: dispatch 的工作组数量向上取整，边缘线程可能超出纹理范围
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }

    // 将像素坐标归一化到 [-1, 1] 范围
    // 这样坐标原点在画面中心，方便数学计算
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) / vec2<f32>(f32(dims.x), f32(dims.y))) * 2.0 - 1.0;

    // 修正宽高比，使圆形不会因为非正方形画布而变成椭圆
    let aspect = f32(dims.x) / f32(dims.y);
    let p = vec2<f32>(uv.x * aspect, uv.y);

    var color = vec3<f32>(0.0);
    var q = p;

    // 4 次迭代叠加，每次对空间进行分形变换并累加光晕颜色
    for (var i = 0; i < 4; i++) {
        // fract(q * 1.5) - 0.5: 将空间重复平铺并居中，产生分形重复图案
        q = fract(q * 1.5) - 0.5;

        // 计算当前点到原点的距离，exp(-length(p)) 让远离中心的区域衰减
        let d = length(q) * exp(-length(p));

        // 通过调色板函数生成颜色，加入迭代偏移和时间变化
        let col = palette(length(p) + f32(i) * 0.4 + params.time * 0.4);

        // sin 波动产生光晕效果，除以 d 在接近零的地方产生高亮
        // abs() 确保亮度为正，0.02 控制整体亮度
        let intensity = abs(sin(d * 8.0 + params.time) / d);
        color += col * intensity * 0.02;
    }

    // 将计算出的颜色写入 storage texture 对应像素
    textureStore(output_texture, gid.xy, vec4<f32>(color, 1.0));
}
