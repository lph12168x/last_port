//! 集成测试: 用 PTY 模拟串口回环
//!
//! PTY 在 Linux 上是标准 tty 设备,serialport-rs 打开后会自动
//! 配置 termios (raw 模式、关闭 echo)。
//!
//! 默认 `#[ignore]`,因为需要 /dev/pts/* 权限,跑法:
//!   cargo test --test integration -- --ignored --nocapture

use last_port_lib::serial::{PortConfig, Session};
use std::os::fd::AsRawFd;

/// 通过 /proc/self/fd/N 解析文件路径 (例如 PTY slave)
fn fd_path(fd: i32) -> std::path::PathBuf {
    std::fs::read_link(format!("/proc/self/fd/{}", fd))
        .expect("failed to readlink /proc/self/fd")
}

#[test]
#[ignore]
fn session_pty_loopback() {
    use nix::pty::openpty;

    // 1. 创建 PTY 对
    let result = openpty(None, None).expect("openpty");
    // OwnedFd → File 才能用 std::io::Read
    let master_file = std::fs::File::from(result.master);
    let slave = result.slave;
    let slave_path = fd_path(slave.as_raw_fd());
    println!("slave path: {:?}", slave_path);

    // 2. 打开串口 (slave 端)
    let cfg = PortConfig::default();
    let session = Session::open(
        slave_path.to_str().expect("slave path utf8"),
        cfg,
    )
    .expect("Session::open");

    // 3. 写入 5 字节
    let payload = b"hello";
    let n = session.write(payload).expect("write");
    assert_eq!(n, payload.len());

    // 4. 从 master 端读出 (raw 模式应无回显,只读到写入的字节)
    let mut master = master_file;
    let mut buf = [0u8; 16];
    let mut total = 0;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while total < payload.len() && std::time::Instant::now() < deadline {
        use std::io::Read;
        match master.read(&mut buf[total..]) {
            Ok(0) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Ok(k) => total += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => panic!("master read err: {}", e),
        }
    }
    assert_eq!(total, payload.len(), "期望读到 {} 字节,实际 {}", payload.len(), total);
    assert_eq!(&buf[..total], payload);

    // 5. 关闭 (drop session)
    drop(session);
}

#[test]
#[ignore]
fn session_open_missing_port() {
    let cfg = PortConfig::default();
    let result = Session::open("/dev/this/does/not/exist", cfg);
    assert!(result.is_err());
    let err = result.unwrap_err();
    // 应该是 PortNotFound 或 Io(NotFound),因平台而异
    let s = err.to_string();
    assert!(
        s.contains("not found") || s.contains("not found"),
        "unexpected error: {}",
        s
    );
}