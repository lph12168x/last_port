//! 串口层统一错误
//!
//! 通过自定义 `Serialize` 把所有错误渲染为字符串发到前端,
//! 前端可以 `try/catch` 后用 `e.toString()` 直接显示。

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SerialError {
    #[error("port not found: {0}")]
    PortNotFound(String),

    #[error("port busy: {0}")]
    PortBusy(String),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("not opened")]
    NotOpened,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialport error: {0}")]
    Serial(#[from] serialport::Error),

    #[error("internal lock poisoned: {0}")]
    Poisoned(String),
}

/// 把错误序列化为字符串,前端 `JSON.stringify(err)` 后拿到 `"port not found: /dev/ttyUSB0"`。
impl Serialize for SerialError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages() {
        assert_eq!(
            SerialError::PortNotFound("/dev/ttyUSB0".into()).to_string(),
            "port not found: /dev/ttyUSB0"
        );
        assert_eq!(SerialError::NotOpened.to_string(), "not opened");
        assert_eq!(
            SerialError::InvalidConfig("bad baud".into()).to_string(),
            "invalid config: bad baud"
        );
    }

    #[test]
    fn serializes_as_string() {
        let err = SerialError::PortBusy("/dev/ttyACM0".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"port busy: /dev/ttyACM0\"");
    }
}