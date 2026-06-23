//! WebGPU 设备初始化模块。
//!
//! 负责建立与 GPU 的连接通道。整个初始化过程:
//! 1. 创建 wgpu Instance（选择 WebGPU 后端）
//! 2. 从 HTML Canvas 创建 Surface（渲染目标）
//! 3. 请求 Adapter（物理 GPU 的抽象）
//! 4. 请求 Device + Queue（逻辑设备和命令队列）
//! 5. 配置 Surface 的像素格式、呈现模式等

/// GPU 初始化的结果，包含后续渲染所需的所有核心对象。
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
}

/// 初始化 WebGPU 并返回核心 GPU 上下文。
///
/// ## 参数
/// - `canvas`: HTML Canvas 元素，作为 GPU 的渲染目标
/// - `width`, `height`: 画布的初始像素尺寸
///
/// ## WebGPU 初始化流程
///
/// ```text
/// Instance → Surface → Adapter → Device/Queue → Surface Config
/// (后端选择)  (渲染目标) (物理GPU)  (逻辑设备)     (格式/呈现方式)
/// ```
///
/// ## Panics
/// - 找不到支持 WebGPU 的 GPU adapter 时
/// - 无法创建逻辑设备时
pub async fn init_gpu(
    canvas: web_sys::HtmlCanvasElement,
    width: u32,
    height: u32,
) -> GpuContext {
    // wgpu::Instance 是与 GPU 系统通信的入口。
    // `BROWSER_WEBGPU` 表示使用浏览器原生的 WebGPU API（而非 WebGL 回退）。
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        flags: Default::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: None,
    });

    // Surface 是 GPU 输出图像的目标，绑定到 HTML Canvas 元素。
    // 在桌面平台上这会绑定到操作系统窗口。
    let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
    let surface = instance
        .create_surface(surface_target)
        .expect("Failed to create surface");

    // Adapter 代表一个物理 GPU。`HighPerformance` 偏好独立显卡（而非集成显卡），
    // `compatible_surface` 确保选出的 GPU 能渲染到我们的 canvas。
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find adapter");

    log::info!("Adapter: {:?}", adapter.get_info());

    // Device 是逻辑设备，是我们提交 GPU 命令的接口。
    // Queue 用于提交命令缓冲区和写入 buffer 数据。
    // `downlevel_webgl2_defaults()` 使用较保守的 limits，确保在大多数 WebGPU 实现上兼容。
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .expect("Failed to create device");

    // 查询 surface 支持的像素格式，优先选择 sRGB 格式以获得正确的伽马校正。
    // sRGB 格式会自动将线性颜色空间转换为适合显示器的 gamma 2.2 曲线。
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(surface_caps.formats[0]);

    // 配置 surface 的渲染参数。
    // - `RENDER_ATTACHMENT`: 表示这个 surface 可以作为 render pass 的颜色附件
    // - `AutoVsync`: 自动启用垂直同步，避免画面撕裂
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);

    GpuContext {
        device,
        queue,
        surface,
        surface_config,
    }
}
