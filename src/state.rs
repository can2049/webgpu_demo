//! 应用状态管理模块。
//!
//! `State` 是整个渲染应用的核心状态容器，持有所有 GPU 资源的所有权。
//! 它负责:
//! - 组装初始化阶段创建的各个 GPU 资源
//! - 处理窗口 resize 时的资源重建
//! - 每帧执行 compute + render 的渲染流程
//!
//! `State` 本身是平台无关的——它只依赖 wgpu 对象，不依赖 winit 或 web_sys。
//! 平台差异在 `gpu.rs` 的初始化函数中处理，结果通过 `GpuContext` 传入。

use crate::gpu::GpuContext;
use crate::params::{self, Params};
use crate::pipeline;
use crate::texture;

/// 渲染应用的完整状态（平台无关）。
///
/// 所有 GPU 资源的生命周期都与 `State` 绑定。
/// - Native: 由 `App` (ApplicationHandler) 持有
/// - WASM: 通过 `Rc<RefCell<State>>` 在 JS 回调间共享
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
    /// 从已初始化的 GPU 上下文创建渲染状态。
    ///
    /// `GpuContext` 封装了平台相关的初始化结果（device, queue, surface），
    /// 此函数在此基础上创建平台无关的渲染资源:
    ///
    /// ```text
    /// GpuContext (来自 gpu.rs，平台相关)
    ///     ↓
    /// Pipelines (compute + render)
    ///     ↓
    /// Params Buffer + Sampler
    ///     ↓
    /// Texture + Bind Groups
    /// ```
    pub fn new(ctx: GpuContext, width: u32, height: u32) -> Self {
        let GpuContext {
            device,
            queue,
            surface,
            surface_config,
        } = ctx;

        let pipeline::ComputeResources {
            pipeline: compute_pipeline,
            bind_group_layout: compute_bind_group_layout,
        } = pipeline::create_compute_pipeline(&device);

        let pipeline::RenderResources {
            pipeline: render_pipeline,
            bind_group_layout: render_bind_group_layout,
        } = pipeline::create_render_pipeline(&device, surface_config.format);

        let params_buffer = params::create_params_buffer(&device, width, height);
        let sampler = texture::create_sampler(&device);

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

    /// 处理窗口/画布尺寸变化。
    ///
    /// 当窗口 resize 时需要:
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
    /// 拆成两个 encoder 并分别 submit，利用队列提交之间的隐式屏障
    /// 确保 compute 写入在 render 读取之前完全可见。
    pub fn render(&self, time: f32) {
        self.queue.write_buffer(
            &self.params_buffer,
            0,
            bytemuck::cast_slice(&[Params::new(time, self.width as f32, self.height as f32)]),
        );

        // ═══ 阶段 1: Compute Pass ═══
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
            self.queue.submit(std::iter::once(encoder.finish()));
        }

        // ═══ 阶段 2: Render Pass ═══
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
                render_pass.draw(0..4, 0..1);
            }
            self.queue.submit(std::iter::once(encoder.finish()));
        }

        surface_texture.present();
    }
}
