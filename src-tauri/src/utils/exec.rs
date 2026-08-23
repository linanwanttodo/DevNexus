// src-tauri/src/utils/exec.rs
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// 统一命令执行：带超时、UTF-8 输出归一化、错误信息结构化
#[derive(Debug)]
pub struct CmdResult {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

/// 执行命令并等待完成，超时则强制终止子进程并返回错误。
///
/// - Unix：使用 `wait-timeout` 精确等待，超时后 `kill()` 杀掉子进程（M1/L3 修复：
///   此前 `recv_timeout` 超时后线程与子进程继续存活，反复超时会累积僵尸进程）。
/// - Windows：`wait-timeout` 不可用，退化为阻塞等待（保留旧行为，超时仅报错）。
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<CmdResult, String> {
    #[cfg(unix)]
    {
        run_unix(program, args, timeout)
    }
    #[cfg(not(unix))]
    {
        run_threaded(program, args, timeout)
    }
}

/// Unix 实现：wait-timeout + kill
#[cfg(unix)]
fn run_unix(program: &str, args: &[&str], timeout: Duration) -> Result<CmdResult, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use wait_timeout::ChildExt;

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to execute '{}': {}", program, e))?;

    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            // 超时：强制终止并回收，避免子进程/管道泄漏
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "command '{}' timed out after {:?} and was terminated",
                program, timeout
            ));
        }
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("failed to wait for '{}': {}", program, e));
        }
    };

    // 读取剩余输出（wait_timeout 已回收进程，直接读管道）
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut so) = child.stdout.take() {
        let _ = so.read_to_string(&mut stdout);
    }
    if let Some(mut se) = child.stderr.take() {
        let _ = se.read_to_string(&mut stderr);
    }

    Ok(CmdResult {
        stdout,
        stderr,
        status: status.code().unwrap_or(-1),
    })
}

/// 非 Unix（Windows）实现：线程 + recv_timeout（保留旧行为）
#[cfg(not(unix))]
fn run_threaded(program: &str, args: &[&str], timeout: Duration) -> Result<CmdResult, String> {
    use std::process::Command;

    let (tx, rx) = std::sync::mpsc::channel();
    let program_owned = program.to_string();
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let output = Command::new(&program_owned).args(&args_owned).output();
        let _ = tx.send(output);
    });
    let output = rx
        .recv_timeout(timeout)
        .map_err(|_| format!("command '{}' timed out after {:?}", program, timeout))?
        .map_err(|e| format!("failed to execute '{}': {}", program, e))?;
    Ok(CmdResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        status: output.status.code().unwrap_or(-1),
    })
}

/// 便捷版：默认超时 + 失败时拼装错误信息
pub fn run_checked(program: &str, args: &[&str]) -> Result<CmdResult, String> {
    let r = run(program, args, DEFAULT_TIMEOUT)?;
    if r.status != 0 {
        return Err(if r.stderr.trim().is_empty() {
            r.stdout.trim().to_string()
        } else {
            r.stderr.trim().to_string()
        });
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_checked_success() {
        let r = run_checked("sh", &["-c", "echo hello"]).unwrap();
        assert_eq!(r.status, 0);
        assert!(r.stdout.contains("hello"));
    }

    #[test]
    fn test_run_checked_nonexistent_program() {
        let err = run_checked("definitely-not-a-real-binary-xyz", &[]).unwrap_err();
        assert!(err.contains("failed to execute"));
    }

    #[test]
    fn test_run_timeout() {
        let err = run("sh", &["-c", "sleep 5"], Duration::from_millis(200)).unwrap_err();
        assert!(err.contains("timed out"));
    }

    #[cfg(unix)]
    #[test]
    fn test_run_timeout_kills_child() {
        // 验证超时后子进程被终止（不再残留 sleep 进程）
        let marker = format!(
            "/tmp/dnx_exec_marker_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let script = format!(
            "echo started > {}; sleep 30; echo done >> {}",
            marker, marker
        );
        let err = run("sh", &["-c", &script], Duration::from_millis(300)).unwrap_err();
        assert!(err.contains("timed out"), "got: {}", err);

        // 给 kill 传播一点时间，然后确认标记文件存在（子进程确实启动了）
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            std::fs::read_to_string(&marker)
                .map(|s| s.contains("started"))
                .unwrap_or(false),
            "child should have started before timeout"
        );
        // 确认 sleep 进程已被终止：等 1s 后标记文件不应再追加 "done"
        std::thread::sleep(Duration::from_secs(1));
        let content = std::fs::read_to_string(&marker).unwrap_or_default();
        assert!(
            !content.contains("done"),
            "child should be killed on timeout, but kept running: {:?}",
            content
        );
        let _ = std::fs::remove_file(&marker);
    }
}
