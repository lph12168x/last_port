//! 串口抽象层 (MVP 单会话,预留多会话扩展)
//!
//! 设计:
//! - `config`: 串口配置参数 + 校验
//! - `error`: 统一错误类型,前端可序列化
//! - `session`: 单个打开的串口 + 后台读线程
//! - `Manager`: 全局会话状态 (Mutex<Option<Session>>)

mod config;
mod error;
mod session;

pub use config::{DataBits, FlowControl, Parity, PortConfig, StopBits};
pub use error::SerialError;
pub use session::{list_ports, PortInfo, Session};

use std::sync::Mutex;

pub type Result<T> = std::result::Result<T, SerialError>;

/// 全局会话管理器
///
/// MVP 仅支持单会话:打开新端口前必须先关闭当前会话。
/// 用 `Mutex<Option<Session>>` 而非 `HashMap`,避免过度设计。
/// 多会话扩展时替换为 `Mutex<HashMap<PortId, Session>>`。
pub struct Manager {
    inner: Mutex<Option<Session>>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// 打开串口,占用当前会话槽。
    /// 若已有会话打开,返回 `PortBusy` 错误。
    pub fn open(&self, name: &str, cfg: PortConfig) -> Result<PortInfo> {
        cfg.validate().map_err(SerialError::InvalidConfig)?;
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SerialError::Poisoned(e.to_string()))?;
        if guard.is_some() {
            return Err(SerialError::PortBusy(name.to_string()));
        }
        let session = Session::open(name, cfg)?;
        let info = session.info().clone();
        *guard = Some(session);
        Ok(info)
    }

    /// 关闭当前会话 (drop Session)。
    pub fn close(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| SerialError::Poisoned(e.to_string()))?;
        guard.take().ok_or(SerialError::NotOpened)?;
        Ok(())
    }

    /// 写入数据到当前会话。
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| SerialError::Poisoned(e.to_string()))?;
        let session = guard.as_ref().ok_or(SerialError::NotOpened)?;
        session.write(data)
    }

    /// 非阻塞尝试从读线程通道取一帧数据。
    pub fn try_recv(&self) -> Option<Vec<u8>> {
        let guard = self.inner.lock().ok()?;
        guard.as_ref().and_then(|s| s.try_recv())
    }

    pub fn is_open(&self) -> bool {
        self.inner.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    pub fn current_port(&self) -> Option<PortInfo> {
        self.inner.lock().ok().and_then(|g| g.as_ref().map(|s| s.info().clone()))
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_starts_closed() {
        let m = Manager::new();
        assert!(!m.is_open());
        assert!(m.try_recv().is_none());
        assert!(m.current_port().is_none());
    }

    #[test]
    fn manager_write_when_closed_returns_not_opened() {
        let m = Manager::new();
        let err = m.write(b"x").unwrap_err();
        assert!(matches!(err, SerialError::NotOpened));
    }

    #[test]
    fn manager_close_when_closed_returns_not_opened() {
        let m = Manager::new();
        let err = m.close().unwrap_err();
        assert!(matches!(err, SerialError::NotOpened));
    }
}