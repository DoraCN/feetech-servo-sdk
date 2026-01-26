use crate::error::{Result, ServoError};
use bytes::BufMut;

pub const HEADER: u16 = 0xFFFF;

// STS/SMS 关键内存地址
pub const ADDR_TORQUE_ENABLE: u8 = 40;
pub const ADDR_GOAL_POSITION: u8 = 42;
pub const ADDR_PRESENT_POSITION: u8 = 56;

// 指令集
pub enum Instruction {
    Ping = 0x01,
    Read = 0x02,
    Write = 0x03,
    RegWrite = 0x04,
    Action = 0x05,
    SyncWrite = 0x83,
}

/// 计算校验和: ~(ID + Length + Instruction + Params) & 0xFF
fn calculate_checksum(id: u8, length: u8, instruction: u8, params: &[u8]) -> u8 {
    let mut sum: u32 = id as u32 + length as u32 + instruction as u32;
    for &p in params {
        sum += p as u32;
    }
    !(sum as u8)
}

/// 构建发送数据包
pub fn pack_instruction(id: u8, instr: Instruction, params: &[u8]) -> Vec<u8> {
    let length = (params.len() + 2) as u8; // Instruction + Params + Checksum
    let instr_byte = instr as u8;

    let mut buf = Vec::with_capacity(length as usize + 4);
    buf.put_u8(0xFF);
    buf.put_u8(0xFF);
    buf.put_u8(id);
    buf.put_u8(length);
    buf.put_u8(instr_byte);
    buf.extend_from_slice(params);

    let checksum = calculate_checksum(id, length, instr_byte, params);
    buf.put_u8(checksum);

    buf
}

/// 解析并验证响应包
/// 响应格式: [0xFF, 0xFF, ID, Length, Error, Param..., Checksum]
pub fn parse_response(id: u8, data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 6 {
        return Err(ServoError::Protocol("Response too short".into()));
    }

    // 1. 检查帧头
    if data[0] != 0xFF || data[1] != 0xFF {
        return Err(ServoError::Protocol("Invalid header".into()));
    }

    // 2. 检查 ID
    if data[2] != id {
        return Err(ServoError::Protocol(format!(
            "ID mismatch, sent {}, got {}",
            id, data[2]
        )));
    }

    let length = data[3];
    let status = data[4];

    // 3. 检查校验和
    // 响应包的 Instruction 位是 Status 字节
    // Checksum = ~(ID + Length + Status + Params)
    let params = &data[5..data.len() - 1];
    let received_checksum = data[data.len() - 1];

    // 注意：计算响应校验和时，把 status 当作 instruction 位置传入
    let calc_checksum = calculate_checksum(id, length, status, params);

    if received_checksum != calc_checksum {
        return Err(ServoError::ChecksumMismatch {
            id,
            expected: calc_checksum,
            actual: received_checksum,
        });
    }

    // 4. 严格模式：检查硬件错误位 (Status Byte)
    // STS 状态位: Bit0=电压, Bit1=角度, Bit2=过热, Bit3=?, Bit4=过载, Bit5=速度
    if status != 0 {
        let msg = decode_hardware_error(status);
        return Err(ServoError::HardwareError {
            id,
            status_byte: status,
            msg,
        });
    }

    Ok(params.to_vec())
}

fn decode_hardware_error(status: u8) -> String {
    let mut errs = Vec::new();
    if status & 0x01 != 0 {
        errs.push("Input Voltage Error");
    }
    if status & 0x02 != 0 {
        errs.push("Angle Limit Error");
    }
    if status & 0x04 != 0 {
        errs.push("Overheating");
    }
    if status & 0x08 != 0 {
        errs.push("Range Error");
    }
    if status & 0x10 != 0 {
        errs.push("Overload");
    }
    errs.join(", ")
}
