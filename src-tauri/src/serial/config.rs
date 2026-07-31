//! 串口配置参数 + 校验
//!
//! 提供 serde 友好枚举,直接通过 Tauri 命令传到前端。

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    None,
    Odd,
    Even,
}

impl From<Parity> for serialport::Parity {
    fn from(p: Parity) -> Self {
        match p {
            Parity::None => serialport::Parity::None,
            Parity::Odd => serialport::Parity::Odd,
            Parity::Even => serialport::Parity::Even,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FlowControl {
    None,
    Software,
    Hardware,
}

impl From<FlowControl> for serialport::FlowControl {
    fn from(f: FlowControl) -> Self {
        match f {
            FlowControl::None => serialport::FlowControl::None,
            FlowControl::Software => serialport::FlowControl::Software,
            FlowControl::Hardware => serialport::FlowControl::Hardware,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataBits {
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
}

impl From<DataBits> for serialport::DataBits {
    fn from(d: DataBits) -> Self {
        match d {
            DataBits::Five => serialport::DataBits::Five,
            DataBits::Six => serialport::DataBits::Six,
            DataBits::Seven => serialport::DataBits::Seven,
            DataBits::Eight => serialport::DataBits::Eight,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopBits {
    One = 1,
    Two = 2,
}

impl From<StopBits> for serialport::StopBits {
    fn from(s: StopBits) -> Self {
        match s {
            StopBits::One => serialport::StopBits::One,
            StopBits::Two => serialport::StopBits::Two,
        }
    }
}

/// 串口配置
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PortConfig {
    /// 波特率,白名单校验 (见 [`PortConfig::validate`])
    pub baud_rate: u32,
    /// 数据位
    pub data_bits: DataBits,
    /// 停止位
    pub stop_bits: StopBits,
    /// 校验位
    pub parity: Parity,
    /// 流控
    pub flow_control: FlowControl,
    /// 单次 read 阻塞超时 (ms),用于驱动后台读线程的轮询节奏。
    pub read_timeout_ms: u64,
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            baud_rate: 9600,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
            flow_control: FlowControl::None,
            read_timeout_ms: 50,
        }
    }
}

impl PortConfig {
    /// 校验配置合法性。返回 `Err(String)` 携带人读消息。
    ///
    /// 波特率采用白名单 (覆盖绝大多数 UART 硬件 + 常见 USB-Serial 适配器)。
    /// Linux/macOS 上 `serialport-rs` 接受任意 baud,但实际硬件可能不支持。
    pub fn validate(&self) -> Result<(), String> {
        const BAUD_RATES: &[u32] = &[
            110, 300, 600, 1200, 2400, 4800, 9600, 14400, 19200, 38400, 57600, 115200, 128000,
            230400, 256000, 460800, 500000, 576000, 921600, 1000000, 1152000, 1500000, 2000000,
            2500000, 3000000, 3500000, 4000000,
        ];
        if !BAUD_RATES.contains(&self.baud_rate) {
            return Err(format!("unsupported baud_rate: {}", self.baud_rate));
        }
        if self.read_timeout_ms == 0 {
            return Err("read_timeout_ms must be > 0".into());
        }
        Ok(())
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.read_timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        assert!(PortConfig::default().validate().is_ok());
    }

    #[test]
    fn common_bauds_are_valid() {
        for b in [9600, 115200, 921600, 1_000_000, 4_000_000] {
            let cfg = PortConfig {
                baud_rate: b,
                ..PortConfig::default()
            };
            assert!(cfg.validate().is_ok(), "baud {} should be valid", b);
        }
    }

    #[test]
    fn zero_baud_is_rejected() {
        let cfg = PortConfig {
            baud_rate: 0,
            ..PortConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn unknown_baud_is_rejected() {
        let cfg = PortConfig {
            baud_rate: 12345,
            ..PortConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn zero_timeout_is_rejected() {
        let cfg = PortConfig {
            read_timeout_ms: 0,
            ..PortConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn enums_convert_to_serialport() {
        assert_eq!(serialport::Parity::from(Parity::Even), serialport::Parity::Even);
        assert_eq!(
            serialport::FlowControl::from(FlowControl::Hardware),
            serialport::FlowControl::Hardware
        );
        assert_eq!(
            serialport::DataBits::from(DataBits::Eight),
            serialport::DataBits::Eight
        );
        assert_eq!(
            serialport::StopBits::from(StopBits::Two),
            serialport::StopBits::Two
        );
    }
}