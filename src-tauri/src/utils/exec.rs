// src-tauri/src/utils/exec.rs
use std::process::Command;
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// 统一命令执行：带超时、UTF-8 输出归一化、错误信息结构化
#[derive(Debug)]
pub struct CmdResult {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<CmdResult, String> {
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
        // 短超时验证 recv_timeout 生效；后台线程由系统回收，不影响测试退出
        let err = run("sh", &["-c", "sleep 5"], Duration::from_millis(200)).unwrap_err();
        assert!(err.contains("timed out"));
    }
}
