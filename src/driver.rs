use crate::bus::{ControlOp, MotorBus};
use crate::error::{Result, ServoError};
use crate::protocol::v0::{self, Instruction};
use async_trait::async_trait;
use std::f32::consts::PI;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Duration;
use tracing::{info, instrument, trace};

// STS3215 规格
const RESOLUTION: f32 = 4096.0;
const MAX_TICKS: f32 = 4095.0;

/// 飞特协议核心控制器（与 I/O 无关）
///
/// 泛型参数 `S` 可以是任何实现了 `AsyncRead + AsyncWrite + Unpin + Send` 的字节流，
/// 例如 `tokio_serial::SerialStream`（串口）或自定义的 USB 字节流（Android）。
///
/// 通常不直接使用此类型，而是使用具名别名：
/// - 串口：[`FeetechBus`]（需开启 `tokio-serial-impl` feature）
/// - 自定义流：`FeetechController::from_stream(your_stream)`
pub struct FeetechController<S> {
    stream: S,
    read_timeout: Duration,
}

impl<S> FeetechController<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    /// 通用构造器：接受任意满足约束的字节流（依赖反转入口）
    ///
    /// - `stream`: 实现了 `AsyncRead + AsyncWrite + Unpin + Send` 的 I/O 流
    ///
    /// 默认读超时为 20ms。如需自定义，使用 [`FeetechController::with_timeout`]。
    pub fn from_stream(stream: S) -> Self {
        Self {
            stream,
            read_timeout: Duration::from_millis(20),
        }
    }

    /// 带自定义读超时的构造器
    pub fn with_timeout(stream: S, read_timeout: Duration) -> Self {
        Self {
            stream,
            read_timeout,
        }
    }

    /// 显式停机：关闭所有指定舵机的扭矩
    pub async fn shutdown(&mut self, ids: &[u8]) -> Result<()>
    where
        S: Sync,
    {
        info!("Shutting down motors: {:?}", ids);
        self.disable_torque(ids).await
    }

    /// 修改底层 8 位寄存器参数
    pub async fn write_byte(&mut self, id: u8, address: u8, value: u8) -> Result<()> {
        let packet = v0::pack_instruction(id, Instruction::Write, &[address, value]);
        self.transfer(id, &packet, 6).await?;
        Ok(())
    }

    /// 修改底层 16 位寄存器参数（小端序）
    pub async fn write_word(&mut self, id: u8, address: u8, value: u16) -> Result<()> {
        let bytes = value.to_le_bytes();
        let packet = v0::pack_instruction(id, Instruction::Write, &[address, bytes[0], bytes[1]]);
        self.transfer(id, &packet, 6).await?;
        Ok(())
    }

    // --- 私有协议辅助 ---

    async fn transfer(&mut self, id: u8, packet: &[u8], response_len: usize) -> Result<Vec<u8>> {
        self.transfer_with_timeout(id, packet, response_len, self.read_timeout)
            .await
    }

    async fn transfer_with_timeout(
        &mut self,
        id: u8,
        packet: &[u8],
        response_len: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        // 发送前排空 RX 缓冲区
        let mut discard = [0u8; 128];
        let flush_timeout = Duration::from_millis(2);
        while let Ok(Ok(n)) =
            tokio::time::timeout(flush_timeout, self.stream.read(&mut discard)).await
        {
            if n == 0 {
                break;
            }
            trace!("Flushed {} bytes from RX buffer", n);
        }

        trace!("TX -> {:02X?}", packet);
        self.stream.write_all(packet).await?;

        let mut buf = vec![0u8; response_len];
        let read_future = self.stream.read_exact(&mut buf);
        match tokio::time::timeout(timeout, read_future).await {
            Ok(io_res) => io_res?,
            Err(_) => return Err(ServoError::Timeout { id }),
        };

        trace!("RX <- {:02X?}", buf);
        v0::parse_response(id, &buf)
    }

    // 物理单位转换: Radians <-> Ticks
    fn rad_to_tick(rad: f32) -> i16 {
        let normalized = (rad / (2.0 * PI)) + 0.5;
        let tick = (normalized * RESOLUTION).round();
        tick.clamp(0.0, MAX_TICKS) as i16
    }

    fn tick_to_rad(tick: i16) -> f32 {
        let normalized = tick as f32 / MAX_TICKS;
        (normalized - 0.5) * 2.0 * PI
    }
}

#[async_trait]
impl<S> MotorBus for FeetechController<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    #[instrument(skip(self))]
    async fn enable_torque(&mut self, ids: &[u8]) -> Result<()> {
        for &id in ids {
            let packet = v0::pack_instruction(id, Instruction::Write, &[v0::ADDR_TORQUE_ENABLE, 1]);
            self.transfer(id, &packet, 6).await?;
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn disable_torque(&mut self, ids: &[u8]) -> Result<()> {
        for &id in ids {
            let packet = v0::pack_instruction(id, Instruction::Write, &[v0::ADDR_TORQUE_ENABLE, 0]);
            let _ = self.transfer(id, &packet, 6).await;
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn read_position(&mut self, id: u8) -> Result<f32> {
        let pos_raw = self.read_raw_position(id).await?;
        Ok(Self::tick_to_rad(pos_raw))
    }

    #[instrument(skip(self))]
    async fn read_raw_position(&mut self, id: u8) -> Result<i16> {
        let packet = v0::pack_instruction(id, Instruction::Read, &[v0::ADDR_PRESENT_POSITION, 2]);
        let params = self.transfer(id, &packet, 8).await?;
        Ok(i16::from_le_bytes([params[0], params[1]]))
    }

    #[instrument(skip(self))]
    async fn set_middle_position(&mut self, id: u8) -> Result<()> {
        info!("Setting ID {} current position as middle position...", id);
        let packet = v0::pack_instruction(id, Instruction::Write, &[v0::ADDR_TORQUE_ENABLE, 128]);

        let mut last_error = None;
        for attempt in 1..=3 {
            match self
                .transfer_with_timeout(id, &packet, 6, Duration::from_millis(1000))
                .await
            {
                Ok(_) => {
                    info!("ID {} set to middle position successfully on attempt {}.", id, attempt);
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        }
        Err(last_error.unwrap())
    }

    #[instrument(skip(self))]
    async fn sync_set_middle_positions(&mut self, ids: &[u8]) -> Result<()> {
        for &id in ids {
            self.set_middle_position(id).await?;
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn write_goal(&mut self, id: u8, op: ControlOp) -> Result<()> {
        match op {
            ControlOp::Position(rad) => {
                let tick = Self::rad_to_tick(rad);
                let bytes = tick.to_le_bytes();
                let packet = v0::pack_instruction(
                    id,
                    Instruction::Write,
                    &[v0::ADDR_GOAL_POSITION, bytes[0], bytes[1]],
                );
                self.transfer(id, &packet, 6).await?;
            }
            ControlOp::RawEffort(raw_tick) => {
                let rad = Self::tick_to_rad(raw_tick);
                let tick = Self::rad_to_tick(rad);
                let bytes = tick.to_le_bytes();
                let packet = v0::pack_instruction(
                    id,
                    Instruction::Write,
                    &[v0::ADDR_GOAL_POSITION, bytes[0], bytes[1]],
                );
                self.transfer(id, &packet, 6).await?;
            }
            _ => return Err(ServoError::Protocol("Unsupported control mode".into())),
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn sync_read_positions(&mut self, ids: &[u8]) -> Result<Vec<f32>> {
        let mut results = Vec::with_capacity(ids.len());
        for &id in ids {
            results.push(self.read_position(id).await?);
        }
        Ok(results)
    }

    #[instrument(skip(self))]
    async fn sync_read_raw_positions(&mut self, ids: &[u8]) -> Result<Vec<i16>> {
        let mut results = Vec::with_capacity(ids.len());
        for &id in ids {
            results.push(self.read_raw_position(id).await?);
        }
        Ok(results)
    }

    #[instrument(skip(self, commands))]
    async fn sync_write_goals(&mut self, commands: &[(u8, ControlOp)]) -> Result<()> {
        if commands.is_empty() {
            return Ok(());
        }

        let mut params = Vec::new();
        params.push(v0::ADDR_GOAL_POSITION);
        params.push(2);

        for (id, op) in commands {
            if let ControlOp::Position(rad) = op {
                let tick = Self::rad_to_tick(*rad);
                let bytes = tick.to_le_bytes();
                params.push(*id);
                params.push(bytes[0]);
                params.push(bytes[1]);
            } else {
                return Err(ServoError::Protocol(
                    "SyncWrite only supports Position currently".into(),
                ));
            }
        }

        let packet = v0::pack_instruction(0xFE, Instruction::SyncWrite, &params);
        trace!("SyncWrite TX -> {:02X?}", packet);
        self.stream.write_all(&packet).await?;
        // SyncWrite 不返回响应
        Ok(())
    }
}

// --- Feature-gated 串口便捷包装 ---

#[cfg(feature = "tokio-serial-impl")]
use tokio_serial::{SerialPortBuilderExt, SerialStream};

/// 串口总线的具名类型别名（需开启 `tokio-serial-impl` feature，默认开启）
///
/// 等价于 `FeetechController<SerialStream>`，保持向后兼容。
#[cfg(feature = "tokio-serial-impl")]
pub type FeetechBus = FeetechController<SerialStream>;

#[cfg(feature = "tokio-serial-impl")]
impl FeetechController<SerialStream> {
    /// 创建并连接到一个串口设备
    ///
    /// - `path`: 串口设备路径（如 `/dev/ttyUSB0` 或 `COM3`）
    /// - `baud_rate`: 波特率（STS3215 常用 `1_000_000`）
    pub fn new(path: &str, baud_rate: u32) -> Result<Self> {
        let stream = tokio_serial::new(path, baud_rate)
            .timeout(Duration::from_millis(100))
            .open_native_async()?;
        Ok(Self::from_stream(stream))
    }
}
