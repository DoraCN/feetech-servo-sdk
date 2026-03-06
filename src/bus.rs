use crate::error::Result;
use async_trait::async_trait;

/// 舵机控制操作枚举
///
/// 用于向舵机发送不同类型的控制指令。目前主要支持位置控制。
#[derive(Debug, Clone, Copy)]
pub enum ControlOp {
    /// 位置控制
    ///
    /// 参数为目标位置（单位: 弧度）。
    Position(f32),
    /// 速度控制 (预留功能，暂未完全实现)
    ///
    /// 参数为目标速度（单位: 弧度/秒）。
    Velocity(f32),
    /// 原始位置控制 (通过原始数据驱动转换)
    ///
    /// 参数为原始位置或对应数据 (例如 0 ~ 4095)
    RawEffort(i16),
}

/// 核心驱动 Trait (舵机总线接口)
///
/// 定义了与舵机通信的标准异步接口。这是 SDK 的核心抽象，
/// 允许使用者在真实硬件 (`FeetechBus`) 和模拟环境 (`MockBus`) 之间无缝切换。
#[async_trait]
pub trait MotorBus: Send + Sync {
    /// 开启指定舵机的扭矩 (使能/Lock)
    ///
    /// 开启后，舵机会保持当前位置并响应位置控制指令。
    /// - `ids`: 需要开启扭矩的舵机 ID 列表。
    async fn enable_torque(&mut self, ids: &[u8]) -> Result<()>;

    /// 关闭指定舵机的扭矩 (失能/Relax/Shutdown)
    ///
    /// 关闭后，舵机可以被外部力量自由转动。
    /// - `ids`: 需要关闭扭矩的舵机 ID 列表。
    async fn disable_torque(&mut self, ids: &[u8]) -> Result<()>;

    /// 读取单个电机的当前位置
    ///
    /// - `id`: 目标舵机 ID。
    /// - **返回值**: 当前位置（单位: 弧度）。
    async fn read_position(&mut self, id: u8) -> Result<f32>;

    /// 读取单个电机的原始位置数值
    ///
    /// - `id`: 目标舵机 ID。
    /// - **返回值**: 舵机内部使用的原始 Tick 值 (`i16`)，例如 STS3215 为 0 ~ 4095。
    async fn read_raw_position(&mut self, id: u8) -> Result<i16>;

    /// 设置单个舵机的当前物理位置为中位 (零点)
    ///
    /// SDK 会向舵机的特定寄存器（如地址 40）写入标定指令。
    /// 注意：此操作涉及舵机内部 Flash/NVM 写入，耗时较长（约数百毫秒）。
    /// - `id`: 目标舵机 ID。
    async fn set_middle_position(&mut self, id: u8) -> Result<()>;

    /// 批量设置多个舵机的当前物理位置为中位 (零点)
    ///
    /// 在内部会依次对列表中的每个舵机调用对应的设置逻辑，并自动处理必要的延时，
    /// 确保总线通信稳定。
    /// - `ids`: 需要设置中位的舵机 ID 列表。
    async fn sync_set_middle_positions(&mut self, ids: &[u8]) -> Result<()>;

    /// 写入单个电机的控制目标 (如位置指令)
    ///
    /// - `id`: 目标舵机 ID。
    /// - `op`: 控制指令参数，目前主要支持 `ControlOp::Position`。
    async fn write_goal(&mut self, id: u8, op: ControlOp) -> Result<()>;

    /// 批量读取多个电机的位置 (软件级同步)
    ///
    /// 由于部分型号（如 Protocol 0）不支持硬件级的 BulkRead，此接口可能通过循环单个读取实现。
    /// - `ids`: 目标舵机 ID 列表。
    /// - **返回值**: 对应 ID 的位置列表（单位: 弧度，其顺序严格对应传入的 `ids` 列表）。
    async fn sync_read_positions(&mut self, ids: &[u8]) -> Result<Vec<f32>>;

    /// 批量读取多个电机的原始数值 (软件级同步)
    ///
    /// - `ids`: 目标舵机 ID 列表。
    /// - **返回值**: 对应 ID 的原始 Tick 值列表（顺序严格对应传入的 `ids` 列表）。
    async fn sync_read_raw_positions(&mut self, ids: &[u8]) -> Result<Vec<i16>>;

    /// 批量同步写入控制目标 (硬件级同步 - SYNC_WRITE)
    ///
    /// 该指令利用硬件支持的 SyncWrite 功能，在一个串口数据包中同时向多个舵机发送目标指令，
    /// 确保多个舵机能在同一时间点开始动作，极大提升多关节机器人的同步性。
    /// - `commands`: 包含 `(舵机ID, 控制指令)` 的元组列表。
    async fn sync_write_goals(&mut self, commands: &[(u8, ControlOp)]) -> Result<()>;
}
