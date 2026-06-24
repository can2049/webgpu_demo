//! WebGPU 设备初始化模块。
//!
//! 负责建立与 GPU 的连接通道。支持两种平台:
//! - **Native (桌面)**: 通过 winit Window 创建 surface，使用 Vulkan/Metal/DX12 后端
//! - **WASM (浏览器)**: 通过 HTML Canvas 创建 surface，使用 WebGPU 后端
//!
//! 初始化流程:
//! 1. 创建 wgpu Instance（选择 GPU 后端）
//! 2. 创建 Surface（渲染目标）
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

/// 在 Native 桌面平台上初始化 GPU。
///
/// 与 WASM 版本的主要区别:
/// - 使用 `OwnedDisplayHandle` 创建 Instance（让 wgpu 知道当前显示系统是 X11/Wayland/Win32 等）
/// - 使用 `Arc<Window>` 创建 Surface（window 放入 Arc 以获得 `'static` 生命周期）
/// - 后端自动选择: Linux 上用 Vulkan，macOS 用 Metal，Windows 用 DX12
#[cfg(not(target_arch = "wasm32"))]
pub async fn init_gpu_native(
    display: winit::event_loop::OwnedDisplayHandle,
    window: std::sync::Arc<winit::window::Window>,
    width: u32,
    height: u32,
) -> GpuContext {
    // `new_with_display_handle` 让 wgpu 使用操作系统的显示句柄来选择合适的 GPU 后端。
    // 不同于 WASM 的 `BROWSER_WEBGPU`，桌面平台会根据 OS 自动选择 Vulkan/Metal/DX12。
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
        Box::new(display),
    ));

    // 用 Arc<Window> 创建 Surface，Arc 让 window 的引用计数管理确保 `'static` 生命周期。
    // Surface 必须和 Window 活得一样长，Arc 自然保证这一点。
    let surface = instance
        .create_surface(window.clone())
        .expect("Failed to create surface");

    init_gpu_common(instance, surface, width, height).await
}

/// 在 WASM 浏览器平台上初始化 GPU。
///
/// 使用 `BROWSER_WEBGPU` 后端，通过浏览器的 `navigator.gpu` API 与 GPU 通信。
/// Surface 绑定到 HTML Canvas 元素。
#[cfg(target_arch = "wasm32")]
pub async fn init_gpu_wasm(
    canvas: web_sys::HtmlCanvasElement,
    width: u32,
    height: u32,
) -> GpuContext {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        flags: Default::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: None,
    });

    let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
    let surface = instance
        .create_surface(surface_target)
        .expect("Failed to create surface");

    init_gpu_common(instance, surface, width, height).await
}

/// 平台无关的 GPU 初始化逻辑。
///
/// Instance 和 Surface 创建因平台而异，但后续的 adapter、device、surface 配置
/// 在所有平台上是完全相同的。
///
/// ```text
/// Instance + Surface (平台相关，由上层提供)
///     ↓
/// Adapter → Device/Queue → Surface Config (本函数，平台无关)
/// ```
async fn init_gpu_common(
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    width: u32,
    height: u32,
) -> GpuContext {
    // Adapter 代表一个物理 GPU。`HighPerformance` 偏好独立显卡（而非集成显卡），
    // `compatible_surface` 确保选出的 GPU 能渲染到我们的 surface。
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
    // `downlevel_webgl2_defaults()` 使用较保守的 limits，确保在大多数实现上兼容。
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
