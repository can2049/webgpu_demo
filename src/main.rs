//! Native 桌面入口 —— 通过 winit 创建窗口，运行 WebGPU 渲染循环。
//!
//! 这个文件是 `cargo run` 的入口，不依赖浏览器，直接在桌面环境运行。
//! 使用 winit 0.30 的 `ApplicationHandler` trait 处理窗口事件。
//!
//! ## 与 WASM 入口 (lib.rs) 的对比
//!
//! | 方面          | Native (main.rs)              | WASM (lib.rs)                    |
//! |---------------|-------------------------------|----------------------------------|
//! | 窗口系统       | winit Window                  | HTML Canvas                      |
//! | 事件循环       | winit EventLoop               | requestAnimationFrame            |
//! | GPU 后端      | Vulkan / Metal / DX12         | WebGPU (浏览器 API)              |
//! | 时间源         | std::time::Instant            | performance.now() (JS)           |
//! | 渲染状态共享    | App 结构体独占                 | Rc<RefCell<State>>               |
//!
//! 核心渲染逻辑 (`State`) 在两个平台上完全相同。

use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use webgpu_demo::{gpu, state::State};

/// Native 应用状态。
///
/// winit 0.30 要求在 `resumed()` 回调中创建窗口（不能在 `main` 中提前创建），
/// 所以初始状态为 `None`，在 `resumed` 时初始化。
struct App {
    /// 窗口引用，用于在 render 后请求下一帧重绘
    window: Option<Arc<Window>>,
    /// 渲染状态，包含所有 GPU 资源
    state: Option<State>,
    /// 程序启动时间，用于计算动画时间
    start_time: std::time::Instant,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            state: None,
            start_time: std::time::Instant::now(),
        }
    }
}

/// winit 0.30 的事件处理 trait。
///
/// ## 事件流程
///
/// ```text
/// EventLoop::run_app()
///     ↓
/// resumed()          ← 创建 Window + 初始化 GPU + 创建 State
///     ↓
/// window_event() 循环:
///     RedrawRequested  → render() + request_redraw()
///     Resized          → resize()
///     CloseRequested   → exit()
/// ```
impl ApplicationHandler for App {
    /// 应用（重新）启动时调用。首次调用时创建窗口和 GPU 资源。
    ///
    /// winit 要求窗口在此回调中创建，因为:
    /// - Android 上 Surface 可能在 suspend/resume 时销毁和重建
    /// - 桌面平台上首次调用 resumed 就是启动时机
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // 已初始化，跳过重复调用
        }

        let window_attrs = Window::default_attributes()
            .with_title("WebGPU Demo — Rust Native")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let size = window.inner_size();

        // Native 路径: winit Window → GpuContext → State
        // `pollster::block_on` 同步等待异步 GPU 初始化完成（桌面平台可以阻塞）
        let ctx = pollster::block_on(gpu::init_gpu_native(
            event_loop.owned_display_handle(),
            window.clone(),
            size.width,
            size.height,
        ));
        let state = State::new(ctx, size.width, size.height);

        self.window = Some(window.clone());
        self.state = Some(state);
        self.start_time = std::time::Instant::now();

        window.request_redraw();
    }

    /// 处理窗口事件。
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let (Some(window), Some(state)) = (self.window.as_ref(), self.state.as_mut()) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Window closed, exiting");
                event_loop.exit();
            }

            // 每帧重绘: 计算动画时间，调用 State::render()，请求下一帧
            WindowEvent::RedrawRequested => {
                let time = self.start_time.elapsed().as_secs_f32();
                state.render(time);
                window.request_redraw();
            }

            // 窗口 resize: 更新 GPU surface 和纹理尺寸
            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
            }

            _ => {}
        }
    }
}

fn main() {
    // env_logger 将 `log` 输出到终端。
    // 通过 `RUST_LOG=info cargo run` 控制日志级别。
    env_logger::init();

    let event_loop = EventLoop::new().expect("Failed to create event loop");

    // `Poll` 模式: 每次事件循环结束后立即开始下一次迭代（不等待新事件），
    // 适合需要持续渲染的实时应用（游戏、动画）。
    // 对比 `Wait` 模式会在没有事件时挂起线程，适合 GUI 工具。
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
