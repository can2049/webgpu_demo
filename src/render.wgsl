// ========================================================================
// Render Shader: 全屏纹理采样 Blit
// ========================================================================
//
// 这个 shader 负责将 compute shader 输出的 storage texture 绘制到屏幕上。
// 使用"全屏四边形 blit"技术:
//   - Vertex Shader 生成覆盖整个屏幕的四边形（无需顶点缓冲区）
//   - Fragment Shader 从纹理中采样颜色
//
// 这是 GPU 图形编程中最基本的后处理/显示模式。

// 顶点着色器输出 / 片段着色器输入
struct VertexOutput {
    @builtin(position) position: vec4<f32>,  // 裁剪空间坐标，GPU 用于光栅化
    @location(0) uv: vec2<f32>,              // UV 纹理坐标，传递给片段着色器
}

// 顶点着色器: 生成全屏四边形的顶点
//
// 不使用任何顶点缓冲区输入，仅通过 vertex_index (0, 1, 2, 3) 生成坐标。
// 使用 TriangleStrip 图元，4 个顶点组成两个三角形覆盖全屏:
//
//   vertex_index:  0        1        2        3
//   位置 (x,y):  (-1,-1)  (1,-1)  (-1,1)   (1,1)
//
//   组成的三角形: [0,1,2] 和 [1,2,3]
//
//   (-1,1)───────(1,1)
//     │ ╲          │
//     │   ╲        │        ↑ y
//     │     ╲      │        │
//     │       ╲    │        └──→ x
//     │         ╲  │
//   (-1,-1)────(1,-1)
//
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // 通过位运算从 vertex_index 计算 x, y 坐标:
    //   index & 1  →  0, 1, 0, 1  →  x: -1, 1, -1, 1
    //   index >> 1 →  0, 0, 1, 1  →  y: -1, -1, 1, 1
    let x = f32(i32(vertex_index & 1u)) * 2.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 2.0 - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);

    // 将裁剪空间坐标 [-1, 1] 转换为纹理 UV 坐标 [0, 1]
    // y 轴翻转 (1.0 - ...) 因为纹理坐标原点在左上角，而裁剪空间原点在左下角
    out.uv = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));

    return out;
}

// compute shader 输出的纹理，作为只读 texture_2d 绑定
@group(0) @binding(0) var tex: texture_2d<f32>;

// 纹理采样器，使用 Linear 过滤进行双线性插值
@group(0) @binding(1) var tex_sampler: sampler;

// 片段着色器: 从纹理采样颜色
//
// 对每个像素，使用顶点着色器传来的 UV 坐标从纹理中采样。
// textureSample() 会根据采样器的过滤设置（Linear）进行双线性插值，
// 使纹理在缩放时看起来更平滑。
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv);
}
