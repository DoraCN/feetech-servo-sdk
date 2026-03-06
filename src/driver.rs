use crate::bus::{ControlOp, MotorBus};
use crate::error::{Result, ServoError};
use crate::protocol::v0::{self, Instruction};
use async_trait::async_trait;
use std::f32::consts::PI;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Duration;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{info, instrument, trace};

// STS3215 规格
const RESOLUTION: f32 = 4096.0;
const MAX_TICKS: f32 = 4095.0;

pub struct FeetechBus {
    stream: SerialStream,
    read_timeout: Duration,
}

impl FeetechBus {
    pub fn new(path: &str, baud_rate: u32) -> Result<Self> {
        let stream = tokio_serial::new(path, baud_rate)
            .timeout(Duration::from_millis(100))
            .open_native_async()?;

        Ok(Self {
            stream,
            read_timeout: Duration::from_millis(20), // 增加默认超时到 20ms
        })
    }

    /// 显式停机：关闭所有扭矩
    pub async fn shutdown(&mut self, ids: &[u8]) -> Result<()> {
        info!("Shutting down motors: {:?}", ids);
        self.disable_torque(ids).await
    }

    /// [新增] 修改 8位 寄存器 (用于调整 P-Gain 等参数)
    pub async fn write_byte(&mut self, id: u8, address: u8, value: u8) -> Result<()> {
        let packet = v0::pack_instruction(id, Instruction::Write, &[address, value]);
        self.transfer(id, &packet, 6).await?;
        Ok(())
    }

    /// [新增] 修改 16位 寄存器 (用于调整 Max Torque 等)
    pub async fn write_word(&mut self, id: u8, address: u8, value: u16) -> Result<()> {
        let bytes = value.to_le_bytes();
        let packet = v0::pack_instruction(id, Instruction::Write, &[address, bytes[0], bytes[1]]);
        self.transfer(id, &packet, 6).await?;
        Ok(())
    }

    // 私有辅助：发送并接收响应
    async fn transfer(&mut self, id: u8, packet: &[u8], response_len: usize) -> Result<Vec<u8>> {
        self.transfer_with_timeout(id, packet, response_len, self.read_timeout)
            .await
    }

    /// [增强] 带有特定超时的传输，并排空干扰数据
    async fn transfer_with_timeout(
        &mut self,
        id: u8,
        packet: &[u8],
        response_len: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        // [优化] 更激进的 RX 缓冲区排空：循环排空直到无数据
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
        let normalized = (rad / (2.0 * PI)) + 0.5; // Map -PI~PI to 0~1
        let tick = (normalized * RESOLUTION).round();
        tick.clamp(0.0, MAX_TICKS) as i16
    }

    fn tick_to_rad(tick: i16) -> f32 {
        let normalized = tick as f32 / MAX_TICKS;
        (normalized - 0.5) * 2.0 * PI
    }
}

#[async_trait]
impl MotorBus for FeetechBus {
    #[instrument(skip(self))]
    async fn enable_torque(&mut self, ids: &[u8]) -> Result<()> {
        // 简单实现：循环发送。如果追求性能可用 SyncWrite
        for &id in ids {
            let packet = v0::pack_instruction(id, Instruction::Write, &[v0::ADDR_TORQUE_ENABLE, 1]);
            self.transfer(id, &packet, 6).await?; // Write returns status packet (6 bytes)
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn disable_torque(&mut self, ids: &[u8]) -> Result<()> {
        for &id in ids {
            let packet = v0::pack_instruction(id, Instruction::Write, &[v0::ADDR_TORQUE_ENABLE, 0]);
            // Best effort shutdown, ignore errors
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
        // Resp: Header(2)+ID(1)+Len(1)+Err(1)+Param(2)+Sum(1) = 8 bytes
        let params = self.transfer(id, &packet, 8).await?;

        Ok(i16::from_le_bytes([params[0], params[1]]))
    }

    #[instrument(skip(self))]
    async fn set_middle_position(&mut self, id: u8) -> Result<()> {
        info!("Setting ID {} current position as middle position...", id);
        // 根据说明书，写 128 到 40 号地址 (ADDR_TORQUE_ENABLE)
        let packet = v0::pack_instruction(id, Instruction::Write, &[v0::ADDR_TORQUE_ENABLE, 128]);

        // [修复] 对于校准指令，使用极长的超时 (1000ms) 并且增加重试机制
        let mut last_error = None;
        for attempt in 1..=3 {
            match self
                .transfer_with_timeout(id, &packet, 6, Duration::from_millis(1000))
                .await
            {
                Ok(_) => {
                    info!(
                        "ID {} set to middle position successfully on attempt {}.",
                        id, attempt
                    );
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(50)).await; // 微小延迟后重试
                    }
                }
            }
        }
        Err(last_error.unwrap())
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
            _ => return Err(ServoError::Protocol("Unsupported control mode".into())),
        }
        Ok(())
    }

    /// 软件级同步读：由于 Protocol 0 标准无 SyncRead，此处使用串行循环读取
    /// 未来优化：若固件支持 BulkRead 可在此替换
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

    /// 硬件级同步写
    #[instrument(skip(self, commands))]
    async fn sync_write_goals(&mut self, commands: &[(u8, ControlOp)]) -> Result<()> {
        if commands.is_empty() {
            return Ok(());
        }

        // Sync Write 格式: [Addr, Len, ID1, Data1_L, Data1_H, ID2, Data2_L, Data2_H ...]
        let mut params = Vec::new();
        params.push(v0::ADDR_GOAL_POSITION);
        params.push(2); // 每个电机的数据长度

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

        // Broadcast ID 0xFE for SyncWrite
        let packet = v0::pack_instruction(0xFE, Instruction::SyncWrite, &params);

        trace!("SyncWrite TX -> {:02X?}", packet);
        self.stream.write_all(&packet).await?;
        // SyncWrite 不返回响应

        Ok(())
    }
}
