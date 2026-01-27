use clap::Parser;
use feetech_servo_sdk::FeetechBus;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// === 寄存器地址定义 (参考文档 Page 12-13) ===
const ADDR_MODE: u8 = 33;       // 运行模式
const ADDR_SPEED: u8 = 46;      // 运行速度
const ADDR_TORQUE: u8 = 40;     // 扭矩开关
const ADDR_LOCK: u8 = 55;       // 锁标志

#[derive(Parser, Debug)]
#[command(author, version, about = "Omni-Wheel Car Control")]
struct Args {
    #[arg(long, default_value = "/dev/ttyUSB0")]
    port: String,
}

struct OmniCar {
    bus: FeetechBus,
    // 定义轮子 ID
    id_left: u8,  // ID 7
    id_back: u8,  // ID 8
    id_right: u8, // ID 9
}

impl OmniCar {
    pub fn new(bus: FeetechBus) -> Self {
        Self {
            bus,
            id_left: 13,
            id_back: 14,
            id_right: 15,
        }
    }

    /// 初始化电机：切换到“恒速模式” (Wheel Mode)
    pub async fn init(&mut self) -> anyhow::Result<()> {
        let ids = [self.id_left, self.id_back, self.id_right];

        info!("⚙️ 正在初始化底盘电机...");

        for id in ids {
            // 1. 解锁 Flash (写0)
            // 如果不解锁，写入地址 33 会失败
            self.bus.write_byte(id, ADDR_LOCK, 0).await?;

            // 2. 设置运行模式为 1 (恒速模式)
            self.bus.write_byte(id, ADDR_MODE, 1).await?;

            // 3. 重新上锁 Flash (写1)
            self.bus.write_byte(id, ADDR_LOCK, 1).await?;

            // 4. 开启扭矩输出
            self.bus.write_byte(id, ADDR_TORQUE, 1).await?;
        }
        info!("✅ 底盘初始化完成：已切换至轮式模式。");
        Ok(())
    }

    pub async fn move_base(&mut self, vx: f32, vy: f32, omega: f32) -> anyhow::Result<()> {

        // --- 核心修正算法 ---

        // 1. ID 7 (左前轮, 60度位置):
        // 向前跑需要反转(或正转，取决于安装)，负责X和Y分量
        // Vector: (-sin(60), cos(60)) = (-0.866, 0.5)
        let v_left = -0.866 * vx + 0.5 * vy + omega;

        // 2. ID 8 (后轮, 180度位置):
        // 它的轮子横着，负责左右(Y)移动，不负责前后(X)
        // Vector: (0, -1) -> 负Y方向
        let v_back = 0.0 * vx - 1.0 * vy + omega;

        // 3. ID 9 (右前轮, 300度/-60度位置):
        // 负责X和Y分量
        // Vector: (sin(60), cos(60)) = (0.866, 0.5)
        let v_right = 0.866 * vx + 0.5 * vy + omega;

        // 4. 写入电机
        // 注意：如果某个轮子转反了，请在这里给 speed 添加负号 "-"
        self.set_wheel_speed(self.id_left, v_left).await?;
        self.set_wheel_speed(self.id_back, v_back).await?;
        self.set_wheel_speed(self.id_right, v_right).await?;

        Ok(())
    }

    /// 写入单个轮子的速度
    /// 处理飞特舵机特殊的 "Bit 15 方向位" 格式
    async fn set_wheel_speed(&mut self, id: u8, speed_val: f32) -> anyhow::Result<()> {
        // 限制最大速度 (防止过快)
        let limit = 1500.0;
        let clamped = speed_val.clamp(-limit, limit);

        // 转换逻辑：文档 Page 13 说明 "BIT15为方向位"
        // 绝对值作为速度大小
        let magnitude = clamped.abs() as u16;

        // 如果是负数，将第15位 (0x8000) 置 1
        let reg_value = if clamped < 0.0 {
            magnitude | 0x8000
        } else {
            magnitude
        };

        // 写入地址 46 (运行速度)
        self.bus.write_word(id, ADDR_SPEED, reg_value).await?;
        Ok(())
    }

    /// 停车
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.move_base(0.0, 0.0, 0.0).await
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Args::parse();

    let bus = FeetechBus::new(&args.port, 1_000_000)?;
    let mut car = OmniCar::new(bus);

    // 1. 必须先执行初始化，切换模式
    car.init().await?;

    info!("🚀 开始全向移动测试 (按 Ctrl+C 停止)...");

    // === 测试动作序列 ===

    // 1. 前进
    info!(">>> 前进 (Forward)");
    car.move_base(500.0, 0.0, 0.0).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 2. 横向漂移 (左)
    info!(">>> 左平移 (Crab Left)");
    car.move_base(0.0, 500.0, 0.0).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 3. 原地旋转
    info!(">>> 旋转 (Spin)");
    car.move_base(0.0, 0.0, 300.0).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 4. 斜向移动 (左前)
    info!(">>> 斜向移动 (Diagonal)");
    car.move_base(400.0, 400.0, 0.0).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 结束
    info!("🛑 停车");
    car.stop().await?;

    Ok(())
}