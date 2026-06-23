//! 纹理和 bind group 管理模块。
//!
//! 本模块负责创建 GPU 纹理和 bind group。在本项目的渲染架构中:
//!
//! ```text
//! Compute Shader ──写入──▶ Storage Texture ──采样──▶ Render Shader ──输出──▶ Screen
//! ```
//!
//! Storage texture 是 compute shader 和 render shader 之间的数据桥梁:
//! - Compute shader 以 `storage_texture<write>` 模式逐像素写入
//! - Render shader 以 `texture_2d<f32>` + sampler 模式采样读取

/// 创建 storage texture 以及 compute 和 render 两个 bind group。
///
/// ## 什么是 Bind Group？
///
/// Bind group 将 GPU 资源（buffer、texture、sampler）绑定到 shader 中的
/// `@group(N) @binding(M)` 声明上。这是 shader 访问外部数据的方式。
///
/// ## 本项目的 bind group 布局
///
/// **Compute bind group (group 0):**
/// - `@binding(0)`: `params` uniform buffer（时间、分辨率）
/// - `@binding(1)`: `output_texture` storage texture（写入模式）
///
/// **Render bind group (group 0):**
/// - `@binding(0)`: `tex` texture_2d（只读采样）
/// - `@binding(1)`: `tex_sampler` sampler（纹理采样器）
pub fn create_texture_and_bind_groups(
    device: &wgpu::Device,
    params_buffer: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
    compute_bgl: &wgpu::BindGroupLayout,
    render_bgl: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::BindGroup, wgpu::BindGroup) {
    // 创建 2D 纹理，用途标记:
    // - `STORAGE_BINDING`: compute shader 可以直接写入（textureStore）
    // - `TEXTURE_BINDING`: render shader 可以采样读取（textureSample）
    // 格式使用 Rgba8Unorm：每通道 8 位无符号归一化值（0.0 ~ 1.0）
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Storage Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    // TextureView 是对 texture 的"视图"，定义了 shader 如何解读这个纹理的数据。
    // 使用默认描述符表示完整访问整个纹理。
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Compute bind group: 将 uniform buffer 和 storage texture 绑定到 compute shader
    let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Compute Bind Group"),
        layout: compute_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
        ],
    });

    // Render bind group: 将纹理和采样器绑定到 render shader 的 fragment 阶段
    let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Render Bind Group"),
        layout: render_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });

    (texture, compute_bind_group, render_bind_group)
}

/// 创建纹理采样器。
///
/// 采样器定义了 shader 通过 `textureSample()` 读取纹理时的过滤方式。
/// `Linear` 过滤表示在纹理放大/缩小时使用双线性插值，使图像看起来更平滑。
/// 对比之下 `Nearest` 过滤会产生像素化的效果。
pub fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Texture Sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}
