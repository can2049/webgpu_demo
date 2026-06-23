//! GPU 管线（Pipeline）创建模块。
//!
//! Pipeline 是 GPU 执行任务的"配方"，定义了 shader 代码、资源绑定布局和固定功能配置。
//! 创建 pipeline 的开销较大（需要编译 shader），但一旦创建后可以在每帧中复用。
//!
//! 本项目使用两条 pipeline:
//! - **Compute Pipeline**: 运行 compute shader 生成图像
//! - **Render Pipeline**: 运行 vertex + fragment shader 将纹理绘制到屏幕

/// Compute pipeline 所需的资源集合。
pub struct ComputeResources {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

/// Render pipeline 所需的资源集合。
pub struct RenderResources {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

/// 创建 compute pipeline 及其 bind group layout。
///
/// ## Bind Group Layout
///
/// Bind group layout 声明了 shader 期望接收哪些类型的资源，是 pipeline 和 bind group 之间的"合约"。
///
/// ```text
/// binding 0: Uniform Buffer   (Params: time, width, height)
/// binding 1: Storage Texture   (rgba8unorm, write-only)
/// ```
///
/// ## Pipeline Layout
///
/// Pipeline layout 由一组 bind group layout 组成。本 pipeline 只有一个 bind group (group 0)。
pub fn create_compute_pipeline(device: &wgpu::Device) -> ComputeResources {
    let compute_shader = device.create_shader_module(wgpu::include_wgsl!("compute.wgsl"));

    // 定义 compute shader 的资源绑定布局
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Compute BGL"),
        entries: &[
            // binding 0: uniform buffer，存放每帧变化的参数
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 1: storage texture，compute shader 写入图像数据
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    // `entry_point: "main"` 对应 compute.wgsl 中的 `fn main()`
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &compute_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    ComputeResources {
        pipeline,
        bind_group_layout,
    }
}

/// 创建 render pipeline 及其 bind group layout。
///
/// ## Render Pipeline 的组成
///
/// ```text
/// Vertex Shader (vs_main)    →  Rasterizer  →  Fragment Shader (fs_main)  →  Output
/// (生成全屏四边形顶点坐标)       (光栅化)        (采样纹理获取颜色)            (写入surface)
/// ```
///
/// ## Bind Group Layout
///
/// ```text
/// binding 0: Texture2D   (compute shader 输出的纹理)
/// binding 1: Sampler      (纹理采样器)
/// ```
///
/// ## 关键配置
///
/// - **TriangleStrip**: 用 4 个顶点画一个覆盖全屏的四边形（两个三角形）
/// - **BlendState::REPLACE**: 直接覆盖输出颜色，不做混合
pub fn create_render_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> RenderResources {
    let render_shader = device.create_shader_module(wgpu::include_wgsl!("render.wgsl"));

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Render BGL"),
        entries: &[
            // binding 0: 只读纹理，接收 compute shader 生成的图像
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // binding 1: 纹理采样器，控制纹理过滤方式
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &render_shader,
            entry_point: Some("vs_main"),
            buffers: &[], // 不使用顶点缓冲区，顶点坐标在 shader 中通过 vertex_index 计算
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &render_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip, // 4 顶点组成全屏四边形
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None, // 全屏 blit 不需要背面剔除
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None, // 2D 全屏绘制不需要深度测试
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    RenderResources {
        pipeline,
        bind_group_layout,
    }
}
