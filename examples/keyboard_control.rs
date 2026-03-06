use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use feetech_servo_sdk::FeetechBus;
use std::time::{Duration, Instant};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

// === 寄存器地址 ===
const ADDR_MODE: u8 = 33;
const ADDR_SPEED: u8 = 46;
const ADDR_TORQUE: u8 = 40;
const ADDR_LOCK: u8 = 55;

#[derive(Parser, Debug)]
#[command(author, version, about = "Smooth Keyboard Control")]
struct Args {
    #[arg(long, default_value = "/dev/ttyUSB0")]
    port: String,

    /// 移动速度 (0-1500)
    #[arg(long, default_value_t = 800.0)]
    speed: f32,

    /// 旋转速度 (0-1500)
    #[arg(long, default_value_t = 500.0)]
    turn_speed: f32,
}

struct OmniCar {
    bus: FeetechBus,
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

    pub async fn init(&mut self) -> anyhow::Result<()> {
        let ids = [self.id_left, self.id_back, self.id_right];
        for id in ids {
            self.bus.write_byte(id, ADDR_LOCK, 0).await?;
            self.bus.write_byte(id, ADDR_MODE, 1).await?;
            self.bus.write_byte(id, ADDR_LOCK, 1).await?;
            self.bus.write_byte(id, ADDR_TORQUE, 1).await?;
        }
        Ok(())
    }

    /// 修正后的运动学解算 (带防饱和处理)
    pub async fn move_base(&mut self, vx: f32, vy: f32, omega: f32) -> anyhow::Result<()> {
        // 1. 原始计算
        let mut v_left = -0.866 * vx + 0.5 * vy + omega;
        let mut v_back = 0.0 * vx - 1.0 * vy + omega;
        let mut v_right = 0.866 * vx + 0.5 * vy + omega;

        // 2. 防饱和处理 (Normalization)
        // 定义舵机的物理极限速度 (建议留一点余量，比如 3800)
        // 如果你设得太高，防饱和就失效了；设太低会浪费性能。
        const PHYSICAL_LIMIT: f32 = 3800.0;

        // 找出三个轮子中需求速度最大的那个
        let max_v = v_left.abs().max(v_back.abs()).max(v_right.abs());

        // 如果最大需求超过了物理极限，则计算缩放比例
        if max_v > PHYSICAL_LIMIT {
            let scale = PHYSICAL_LIMIT / max_v;
            // 等比例缩小所有轮子的速度
            v_left *= scale;
            v_back *= scale;
            v_right *= scale;

            // 可选：打印警告，让用户知道速度被限制了
            // println!("⚠️ 速度饱和！自动限速: 请求 {:.0} -> 实际 {:.0}", max_v, PHYSICAL_LIMIT);
        }

        // 3. 写入电机
        self.set_speed(self.id_left, v_left).await?;
        self.set_speed(self.id_back, v_back).await?;
        self.set_speed(self.id_right, v_right).await?;
        Ok(())
    }

    async fn set_speed(&mut self, id: u8, speed: f32) -> anyhow::Result<()> {
        // 这里的 limit 必须和上面的 PHYSICAL_LIMIT 一致或更大
        // 现在的逻辑主要靠上面的 normalization 来限制，这里作为最后一道保险
        let limit = 4095.0;
        let clamped = speed.clamp(-limit, limit);

        let magnitude = clamped.abs() as u16;
        let reg_val = if clamped < 0.0 {
            magnitude | 0x8000
        } else {
            magnitude
        };
        self.bus.write_word(id, ADDR_SPEED, reg_val).await?;
        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.move_base(0.0, 0.0, 0.0).await
    }
}

// 定义按键动作枚举
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Stop,
    Forward,
    Backward,
    Left,
    Right,
    TurnLeft,
    TurnRight,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Args::parse();

    println!("正在连接底盘...");
    let bus = FeetechBus::new(&args.port, 1_000_000)?;
    let mut car = OmniCar::new(bus);
    car.init().await?;

    println!("============================================");
    println!("  🎮 平滑键盘控制 (Smooth Mode)");
    println!("--------------------------------------------");
    println!("  长按 [W/A/S/D] 移动");
    println!("  长按 [Q/E] 旋转");
    println!("  松开按键自动停止");
    println!("  [ESC] / [Ctrl+C] 退出");
    println!("============================================");

    enable_raw_mode()?;

    // 控制循环频率：50Hz (每 20ms 发送一次指令)
    let mut interval = tokio::time::interval(Duration::from_millis(20));

    // 状态记录
    let mut current_action = Action::Stop;
    let mut last_key_time = Instant::now();
    // 按键超时时间：300ms
    // 如果超过这个时间没有检测到按键Press/Repeat事件，就认为用户松手了
    // 这个值需要比系统的按键重复延迟(通常500ms)略大，或者依靠Repeat事件刷新
    // 在Raw Mode下，长按会产生连续的Event，间隔通常在30-100ms之间
    let key_timeout = Duration::from_millis(300);

    loop {
        // 1. 等待下一个控制周期 (保证频率稳定)
        interval.tick().await;

        // 2. 非阻塞读取所有积压的按键事件
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()?
                && (key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat)
            {
                last_key_time = Instant::now(); // 刷新活跃时间

                match key.code {
                    KeyCode::Char('w') | KeyCode::Char('W') => current_action = Action::Forward,
                    KeyCode::Char('s') | KeyCode::Char('S') => current_action = Action::Backward,
                    KeyCode::Char('a') | KeyCode::Char('A') => current_action = Action::Left,
                    KeyCode::Char('d') | KeyCode::Char('D') => current_action = Action::Right,
                    KeyCode::Char('q') | KeyCode::Char('Q') => current_action = Action::TurnLeft,
                    KeyCode::Char('e') | KeyCode::Char('E') => current_action = Action::TurnRight,
                    KeyCode::Esc => {
                        disable_raw_mode()?;
                        car.stop().await?;
                        println!("退出.");
                        return Ok(());
                    }
                    KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                        disable_raw_mode()?;
                        car.stop().await?;
                        return Ok(());
                    }
                    _ => {} // 忽略其他按键
                }
            }
        }

        // 3. 超时检查 (判断是否松手)
        if last_key_time.elapsed() > key_timeout {
            current_action = Action::Stop;
        }

        // 4. 根据当前动作计算速度
        let (vx, vy, omega) = match current_action {
            Action::Forward => (args.speed, 0.0, 0.0),
            Action::Backward => (-args.speed, 0.0, 0.0),
            Action::Left => (0.0, args.speed, 0.0),
            Action::Right => (0.0, -args.speed, 0.0),
            Action::TurnLeft => (0.0, 0.0, args.turn_speed),
            Action::TurnRight => (0.0, 0.0, -args.turn_speed),
            Action::Stop => (0.0, 0.0, 0.0),
        };

        // 5. 发送到底盘
        if let Err(_e) = car.move_base(vx, vy, omega).await {
            // 忽略通信错误，保持循环
        }
    }
}
