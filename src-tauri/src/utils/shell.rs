use std::process::Command;

/// Detect the current user's default shell
pub fn detect_shell() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    shell
}

/// Execute a shell command and return its output
pub async fn execute_command(cmd: &str) -> Result<String, String> {
    let shell = detect_shell();
    let output = Command::new(&shell)
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
