//! 传递给 GPU shader 的 uniform 参数。
//!
//! 这个结构体会被写入 GPU buffer，供 compute shader 读取。
//! 它包含动画时间和画布分辨率，shader 用这些参数来生成随时间变化的图像。

use wgpu::util::DeviceExt;

/// GPU uniform buffer 中的参数布局。
///
/// `#[repr(C)]` 确保内存布局与 C 语言一致，这是 GPU buffer 的要求。
/// `bytemuck::Pod` 和 `Zeroable` 让结构体可以安全地转换为字节切片，
/// 用于写入 GPU buffer。
///
/// 对应 WGSL shader 中的:
/// ```wgsl
/// struct Params {
///     time: f32,
///     width: f32,
///     height: f32,
///     _padding: f32,
/// }
/// ```
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Params {
    /// 动画时间（秒），来自 `requestAnimationFrame` 的时间戳
    pub time: f32,
    /// 画布宽度（像素），用于 shader 中计算 UV 坐标和宽高比
    pub width: f32,
    /// 画布高度（像素）
    pub height: f32,
    /// 填充字段，确保结构体大小为 16 字节对齐。
    /// WebGPU 规范要求 uniform buffer 的绑定偏移量按 16 字节对齐。
    pub _padding: f32,
}

impl Params {
    /// 创建一个新的 Params 实例。
    pub fn new(time: f32, width: f32, height: f32) -> Self {
        Self {
            time,
            width,
            height,
            _padding: 0.0,
        }
    }
}

/// 创建 GPU uniform buffer 并写入初始参数。
///
/// buffer 用途标记为 `UNIFORM | COPY_DST`:
/// - `UNIFORM`: 可以在 shader 中作为 uniform 变量绑定
/// - `COPY_DST`: 允许在每帧通过 `queue.write_buffer()` 更新内容
pub fn create_params_buffer(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Params Buffer"),
        contents: bytemuck::cast_slice(&[Params::new(0.0, width as f32, height as f32)]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}
