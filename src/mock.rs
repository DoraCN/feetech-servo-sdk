use crate::bus::{ControlOp, MotorBus};
use crate::error::{Result, ServoError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{info, warn};

/// 模拟舵机的内部状态
#[derive(Debug, Clone)]
struct MockServoState {
    id: u8,
    current_pos: f32, // 弧度
    target_pos: f32,  // 弧度
    torque_enabled: bool,
    last_update: Instant,
}

impl MockServoState {
    fn new(id: u8, pos: f32) -> Self {
        Self {
            id,
            current_pos: pos,
            target_pos: pos,
            torque_enabled: false,
            last_update: Instant::now(),
        }
    }

    /// 简单的物理模拟：根据时间流逝移动电机
    fn update(&mut self) {
        if !self.torque_enabled {
            // 如果没上电，位置不变（或者模拟重力下垂，这里暂时保持不变）
            self.last_update = Instant::now();
            return;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();

        // 模拟最大速度: 假设 5.0 rad/s (~280 deg/s)
        let max_speed = 5.0;
        let step = max_speed * dt;

        if (self.target_pos - self.current_pos).abs() <= step {
            self.current_pos = self.target_pos;
        } else if self.target_pos > self.current_pos {
            self.current_pos += step;
        } else {
            self.current_pos -= step;
        }

        self.last_update = now;
    }
}

/// 纯内存的模拟总线
pub struct MockBus {
    // 使用 Arc<Mutex> 允许内部状态在多次调用间保持
    servos: Arc<Mutex<HashMap<u8, MockServoState>>>,
}

impl MockBus {
    /// 创建一个新的模拟总线，预设一些电机 ID
    pub fn new(ids: &[u8]) -> Self {
        let mut map = HashMap::new();
        for &id in ids {
            map.insert(id, MockServoState::new(id, 0.0));
        }
        info!("MockBus initialized with servos: {:?}", ids);
        Self {
            servos: Arc::new(Mutex::new(map)),
        }
    }

    /// 后门方法：用于测试时强制设置电机状态
    pub fn set_servo_position_instant(&self, id: u8, pos: f32) {
        let mut servos = self.servos.lock().unwrap();
        if let Some(s) = servos.get_mut(&id) {
            s.current_pos = pos;
            s.target_pos = pos;
        }
    }
}

#[async_trait]
impl MotorBus for MockBus {
    async fn enable_torque(&mut self, ids: &[u8]) -> Result<()> {
        let mut servos = self.servos.lock().unwrap();
        for &id in ids {
            if let Some(s) = servos.get_mut(&id) {
                s.torque_enabled = true;
                s.last_update = Instant::now(); // 重置时间戳
                info!("[Mock] Servo {} torque ENABLED", id);
            } else {
                warn!("[Mock] Enable torque failed: Servo {} not found", id);
                // 在 Mock 中我们通常选择不报错，或者根据 Strict 模式报错
                // 这里为了模拟真实硬件找不到 ID 的情况：
                return Err(ServoError::Timeout { id });
            }
        }
        Ok(())
    }

    async fn disable_torque(&mut self, ids: &[u8]) -> Result<()> {
        let mut servos = self.servos.lock().unwrap();
        for &id in ids {
            if let Some(s) = servos.get_mut(&id) {
                s.torque_enabled = false;
                info!("[Mock] Servo {} torque DISABLED", id);
            }
        }
        Ok(())
    }

    async fn read_position(&mut self, id: u8) -> Result<f32> {
        let tick = self.read_raw_position(id).await?;
        let max_ticks = 4095.0;
        let pi = std::f32::consts::PI;
        let normalized = tick as f32 / max_ticks;
        Ok((normalized - 0.5) * 2.0 * pi)
    }

    async fn read_raw_position(&mut self, id: u8) -> Result<i16> {
        let mut servos = self.servos.lock().unwrap();
        if let Some(s) = servos.get_mut(&id) {
            s.update();
            let rad = s.current_pos;
            let resolution = 4096.0;
            let max_ticks = 4095.0;
            let pi = std::f32::consts::PI;

            let normalized = (rad / (2.0 * pi)) + 0.5;
            let tick = (normalized * resolution).round();
            Ok(tick.clamp(0.0, max_ticks) as i16)
        } else {
            Err(ServoError::Timeout { id })
        }
    }

    async fn set_middle_position(&mut self, id: u8) -> Result<()> {
        let mut servos = self.servos.lock().unwrap();
        if let Some(s) = servos.get_mut(&id) {
            info!(
                "[Mock] Setting ID {} current position as middle (reset to 0.0 rad)",
                id
            );
            s.current_pos = 0.0;
            s.target_pos = 0.0;
            s.last_update = std::time::Instant::now();
            Ok(())
        } else {
            Err(ServoError::Timeout { id })
        }
    }

    // 重新实现 read_position 以调用 read_raw_position 保持一致（可选）
    // 或者直接保持原来的样子，但为了最佳实践，Mock 应该反映硬件

    async fn write_goal(&mut self, id: u8, op: ControlOp) -> Result<()> {
        let mut servos = self.servos.lock().unwrap();
        if let Some(s) = servos.get_mut(&id) {
            match op {
                ControlOp::Position(rad) => {
                    s.target_pos = rad;
                    // 如果没上电，虽然写入了目标，但 update 时不会动
                }
                _ => warn!("[Mock] Only Position control is supported in Mock currently"),
            }
            Ok(())
        } else {
            Err(ServoError::Timeout { id })
        }
    }

    async fn sync_read_positions(&mut self, ids: &[u8]) -> Result<Vec<f32>> {
        let mut results = Vec::new();
        for &id in ids {
            results.push(self.read_position(id).await?);
        }
        Ok(results)
    }

    async fn sync_read_raw_positions(&mut self, ids: &[u8]) -> Result<Vec<i16>> {
        let mut results = Vec::new();
        for &id in ids {
            results.push(self.read_raw_position(id).await?);
        }
        Ok(results)
    }

    async fn sync_write_goals(&mut self, commands: &[(u8, ControlOp)]) -> Result<()> {
        let mut servos = self.servos.lock().unwrap();
        for (id, op) in commands {
            if let Some(s) = servos.get_mut(id) {
                if let ControlOp::Position(rad) = op {
                    s.target_pos = *rad;
                }
            }
        }
        Ok(())
    }
}
