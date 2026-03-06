use crate::error::Result;
use async_trait::async_trait;

/// 舵机控制操作枚举
#[derive(Debug, Clone, Copy)]
pub enum ControlOp {
    /// 位置控制 (单位: 弧度)
    Position(f32),
    /// 速度控制 (单位: 弧度/秒) - 预留
    Velocity(f32),
    /// 原始 PWM/力矩 (-1000 ~ 1000) - 预留
    RawEffort(i16),
}

/// 核心驱动 Trait
#[async_trait]
pub trait MotorBus: Send + Sync {
    /// 开启扭矩 (Lock)
    async fn enable_torque(&mut self, ids: &[u8]) -> Result<()>;

    /// 关闭扭矩 (Relax/Shutdown)
    async fn disable_torque(&mut self, ids: &[u8]) -> Result<()>;

    /// 读取单个电机位置 (返回弧度)
    async fn read_position(&mut self, id: u8) -> Result<f32>;

    /// 读取单个电机原始位置 (返回 Ticks, i16)
    async fn read_raw_position(&mut self, id: u8) -> Result<i16>;

    /// 设置当前位置为中位 (写入 128 到地址 40)
    async fn set_middle_position(&mut self, id: u8) -> Result<()>;

    /// 批量设置中位
    async fn sync_set_middle_positions(&mut self, ids: &[u8]) -> Result<()>;

    /// 写入单个电机目标
    async fn write_goal(&mut self, id: u8, op: ControlOp) -> Result<()>;

    /// 批量同步读取 (软件级实现)
    /// 返回值 Vec<f32> 顺序严格对应传入的 ids
    async fn sync_read_positions(&mut self, ids: &[u8]) -> Result<Vec<f32>>;

    /// 批量同步读取原始数值 (软件级实现)
    /// 返回值 Vec<i16> 顺序严格对应传入的 ids
    async fn sync_read_raw_positions(&mut self, ids: &[u8]) -> Result<Vec<i16>>;

    /// 批量同步写入 (SYNC_WRITE 指令)
    async fn sync_write_goals(&mut self, commands: &[(u8, ControlOp)]) -> Result<()>;
}
