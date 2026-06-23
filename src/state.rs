//! 应用状态管理模块。
//!
//! `State` 是整个渲染应用的核心状态容器，持有所有 GPU 资源的所有权。
//! 它负责:
//! - 组装初始化阶段创建的各个 GPU 资源
//! - 处理窗口 resize 时的资源重建
//! - 每帧执行 compute + render 的渲染流程

use crate::gpu;
use crate::params::{self, Params};
use crate::pipeline;
use crate::texture;

/// 渲染应用的完整状态。
///
/// 所有 GPU 资源的生命周期都与 `State` 绑定。
/// 在 WASM 中通过 `Rc<RefCell<State>>` 共享，因为 JavaScript 回调需要多处访问。
pub struct State {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    params_buffer: wgpu::Buffer,
    compute_bind_group_layout: wgpu::BindGroupLayout,
    render_bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    storage_texture: wgpu::Texture,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
}

impl State {
    /// 创建并初始化完整的渲染状态。
    ///
    /// 初始化顺序很重要，因为后续步骤依赖前面创建的资源:
    ///
    /// ```text
    /// GPU Context (device, queue, surface)
    ///     ↓
    /// Pipelines (compute + render)
    ///     ↓
    /// Params Buffer + Sampler
    ///     ↓
    /// Texture + Bind Groups
    /// ```
    pub async fn new(canvas: web_sys::HtmlCanvasElement, width: u32, height: u32) -> Self {
        // 步骤 1: 初始化 GPU 设备和 surface
        let gpu::GpuContext {
            device,
            queue,
            surface,
            surface_config,
        } = gpu::init_gpu(canvas, width, height).await;

        // 步骤 2: 创建渲染管线
        let pipeline::ComputeResources {
            pipeline: compute_pipeline,
            bind_group_layout: compute_bind_group_layout,
        } = pipeline::create_compute_pipeline(&device);

        let pipeline::RenderResources {
            pipeline: render_pipeline,
            bind_group_layout: render_bind_group_layout,
        } = pipeline::create_render_pipeline(&device, surface_config.format);

        // 步骤 3: 创建 uniform buffer 和纹理采样器
        let params_buffer = params::create_params_buffer(&device, width, height);
        let sampler = texture::create_sampler(&device);

        // 步骤 4: 创建纹理和 bind groups
        let (storage_texture, compute_bind_group, render_bind_group) =
            texture::create_texture_and_bind_groups(
                &device,
                &params_buffer,
                &sampler,
                &compute_bind_group_layout,
                &render_bind_group_layout,
                width,
                height,
            );

        Self {
            device,
            queue,
            surface,
            surface_config,
            compute_pipeline,
            render_pipeline,
            params_buffer,
            compute_bind_group_layout,
            render_bind_group_layout,
            storage_texture,
            compute_bind_group,
            render_bind_group,
            sampler,
            width,
            height,
        }
    }

    /// 处理画布尺寸变化。
    ///
    /// 当浏览器窗口 resize 时需要:
    /// 1. 更新 surface 配置（告诉 GPU 新的输出尺寸）
    /// 2. 重建 storage texture（因为旧纹理尺寸不匹配）
    /// 3. 重建 bind groups（因为它们引用了旧的 texture view）
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        let (texture, compute_bg, render_bg) = texture::create_texture_and_bind_groups(
            &self.device,
            &self.params_buffer,
            &self.sampler,
            &self.compute_bind_group_layout,
            &self.render_bind_group_layout,
            width,
            height,
        );
        self.storage_texture = texture;
        self.compute_bind_group = compute_bg;
        self.render_bind_group = render_bg;
    }

    /// 执行一帧的渲染。
    ///
    /// 每帧渲染流程:
    ///
    /// ```text
    /// 1. 更新 uniform buffer    ──  将当前时间和分辨率写入 GPU
    /// 2. Compute Pass           ──  GPU 并行计算每像素颜色 → storage texture
    ///    (独立 encoder + submit，确保 compute 写入完成)
    /// 3. 获取 surface texture   ──  从交换链获取一帧的输出目标
    /// 4. Render Pass            ──  全屏四边形采样 storage texture → surface
    ///    (独立 encoder + submit)
    /// 5. 呈现到屏幕
    /// ```
    ///
    /// ## 为什么拆成两个 CommandEncoder？
    ///
    /// Compute pass 写入 storage texture，render pass 从中读取。
    /// 这需要 GPU 上的 pipeline barrier 来完成存储写入→纹理读取的同步。
    ///
    /// 在同一个 encoder 内，Dawn 的 Vulkan 后端可能不会正确插入
    /// `VkImageMemoryBarrier`，导致 render pass 读到未定义数据（黑屏）。
    ///
    /// 拆成两个 encoder 并分别 submit，利用队列提交之间的隐式屏障
    /// 确保 compute 写入在 render 读取之前完全可见。这在所有后端
    /// （Vulkan、OpenGL、Metal、D3D12）上行为一致。
    pub fn render(&self, time: f32) {
        // 更新 uniform buffer: 将新的时间和分辨率写入 GPU
        self.queue.write_buffer(
            &self.params_buffer,
            0,
            bytemuck::cast_slice(&[Params::new(time, self.width as f32, self.height as f32)]),
        );

        // ═══════════════════════════════════════════════════════════
        // 阶段 1: Compute Pass（独立 CommandEncoder）
        // ═══════════════════════════════════════════════════════════
        // workgroup 大小为 16x16，所以需要 ceil(width/16) x ceil(height/16) 个工作组
        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Compute Encoder"),
                });
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Compute Pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
                let wg_x = (self.width + 15) / 16;
                let wg_y = (self.height + 15) / 16;
                compute_pass.dispatch_workgroups(wg_x, wg_y, 1);
            }
            // 提交 compute 工作，队列屏障保证 storage texture 写入完成
            self.queue.submit(std::iter::once(encoder.finish()));
        }

        // ═══════════════════════════════════════════════════════════
        // 阶段 2: Render Pass（独立 CommandEncoder）
        // ═══════════════════════════════════════════════════════════
        // 使用全屏四边形（4 顶点的 triangle strip）进行纹理采样 blit

        // 从 swap chain 获取当前帧的 surface texture。
        // 可能的失败情况: 超时、遮挡（窗口被盖住）、surface 失效等。
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost
            | wgpu::CurrentSurfaceTexture::Validation => {
                log::warn!("Surface texture unavailable, skipping frame");
                return;
            }
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, &self.render_bind_group, &[]);
                render_pass.draw(0..4, 0..1); // 4 个顶点 = 一个全屏四边形
            }
            // 提交 render 工作
            self.queue.submit(std::iter::once(encoder.finish()));
        }

        surface_texture.present();
    }
}
