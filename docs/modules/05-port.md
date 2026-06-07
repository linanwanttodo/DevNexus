# 端口管理 — 模块设计文档

## 1. 功能概述

端口管理器（Port Manager）列出当前系统所有监听的 TCP/UDP 端口，显示占用进程信息，支持一键杀死占用端口的进程。

**通信链路**:
```
PortManager.svelte ──→ invoke("list_ports")  ──→ port_manager.rs
                  ──→ invoke("kill_port")    ──→ port_manager.rs
```

---

## 2. 数据结构

```rust
#[derive(Serialize, Debug)]
pub struct PortEntry {
    pub port: u16,              // 3000
    pub process_name: String,   // "node"
    pub pid: u32,               // 12345
    pub protocol: String,       // "TCP" / "UDP"
    pub state: String,          // "LISTEN" / "ESTABLISHED"
}
```

**前端对应** (`routes/PortManager.svelte`):

```javascript
let ports = $state([]);
let search = $derived(getSearchQuery());

let filtered = $derived(
    search.trim()
        ? ports.filter(p =>
            p.port.toString().includes(search) ||
            p.process_name.toLowerCase().includes(search.toLowerCase()) ||
            p.pid.toString().includes(search)
          )
        : ports
);
```

---

## 3. 核心实现 — 跨平台端口枚举

这是全项目跨平台实现差异最大的模块之一。三个平台的端口列表命令和输出格式完全不同。

### 3.1 macOS — lsof

```rust
#[cfg(target_os = "macos")]
fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    // lsof -iTCP -sTCP:LISTEN -P -n -F pcfn
    // -iTCP: 只列 TCP
    // -sTCP:LISTEN: 只列 LISTEN 状态
    // -P: 不解析端口名（直接显示端口号）
    // -n: 不解析主机名
    // -F: 机器可解析的输出格式

    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-P", "-n", "-Fpcfn"])
        .output()
        .map_err(|e| format!("Failed to run lsof: {}", e))?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    // 解析 -F 格式: p<PID> c<NAME> f<FD> n<ADDRESS>
    // 例: p12345\ncnode\nf123\nn*:3000 (LISTEN)
}
```

**解析逻辑**:
```
p12345     → PID=12345
cnode      → process_name=node
n*:3000    → port=3000
```

### 3.2 Linux — /proc/net/tcp

```rust
#[cfg(target_os = "linux")]
fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    // 直接读取 /proc/net/tcp — 最快，零外部依赖
    let content = std::fs::read_to_string("/proc/net/tcp")
        .map_err(|e| format!("Cannot read /proc/net/tcp: {}", e))?;

    let mut entries = Vec::new();
    for line in content.lines().skip(1) {  // 跳过标题行
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }

        // 本地地址格式: 00000000:0BB8 (16进制)
        let local = parts[1];
        let hex_parts: Vec<&str> = local.split(':').collect();
        if hex_parts.len() != 2 { continue; }

        let port = u16::from_str_radix(hex_parts[1], 16).unwrap_or(0);
        let state = parts[3];  // 0A = LISTEN

        // 从 /proc/net/tcp 本身拿不到进程名，需要额外读取
        // 从 inode 反向映射到 fd 再找到 PID
        let inode = parts[9];
        let pid = find_pid_by_inode(inode);
        let process_name = find_process_name(pid);

        entries.push(PortEntry { port, process_name, pid, protocol: "TCP".into(), state: "LISTEN".into() });
    }
    // 同样的方式读取 /proc/net/tcp6 获取 IPv6 端口
}
```

**Linux 方案的特殊设计**:
- `lsof` 在 Linux 上通常不可用（需手动安装），所以选择 `/proc/net/tcp`
- `/proc/net/tcp` 只包含端口号（16 进制）和 inode，不包含进程名
- 进程名需要通过 inode → `/proc/[pid]/fd/` → socket 匹配来反向查找

```rust
fn find_pid_by_inode(target_inode: &str) -> u32 {
    // 遍历 /proc/[pid]/fd/ 下的所有 symlink
    // 如果指向 "socket:[inode]" 则匹配
    for entry in std::fs::read_dir("/proc").ok()? {
        let pid = entry.file_name().to_str()?.parse::<u32>().ok()?;
        // 读取 /proc/[pid]/fd/ 下的所有文件描述符
    }
}
```

### 3.3 Windows — netstat

```rust
#[cfg(target_os = "windows")]
fn list_ports_impl() -> Result<Vec<PortEntry>, String> {
    // netstat -ano 显示所有连接和监听端口
    // -a: 所有连接
    // -n: 数字格式
    // -o: 关联 PID

    let output = Command::new("netstat")
        .args(&["-ano"])
        .output()
        .map_err(|e| format!("Failed to run netstat: {}", e))?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    // 输出格式:
    //   Proto  Local Address          Foreign Address        State           PID
    //   TCP    0.0.0.0:3000          0.0.0.0:0              LISTENING       12345

    for line in output_str.lines() {
        // 解析固定列宽格式
    }
}
```

**三平台端口枚举方案对比**:

| 维度 | macOS | Linux | Windows |
|------|-------|-------|---------|
| 命令/文件 | `lsof` | `/proc/net/tcp` | `netstat -ano` |
| 外部依赖 | 系统内置 | 无（procfs） | 系统内置 |
| 性能 | 较快（~20ms） | 最快（~2ms） | 较慢（~50ms） |
| 进程名 | 直接包含 | 需 inode 反向查找 | 直接包含 |
| 解析难度 | 中等 | 复杂 | 简单 |

---

## 4. 跨平台端口释放

### Unix (macOS/Linux)

```rust
#[cfg(not(target_os = "windows"))]
fn kill_process(pid: u32) -> Result<String, String> {
    // 先发 SIGTERM，等待 2 秒
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_secs(2));

    // 如果进程仍然存活，发送 SIGKILL
    if is_process_alive(pid) {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
        Ok(format!("Process {} forcefully terminated (SIGKILL)", pid))
    } else {
        Ok(format!("Process {} gracefully terminated (SIGTERM)", pid))
    }
}
```

**为什么两步杀**:
- `SIGTERM` (信号 15): 允许进程优雅退出，清理资源（关闭文件描述符、刷新缓冲区等）
- `SIGKILL` (信号 9): 直接杀死，不等待清理。用作最后手段

### Windows

```rust
#[cfg(target_os = "windows")]
fn kill_process(pid: u32) -> Result<String, String> {
    // taskkill /F 强制终止
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|e| format!("Failed to kill process {}: {}", pid, e))?;

    if output.status.success() {
        Ok(format!("Process {} terminated", pid))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
```

Windows 使用 `taskkill /F` 强制终止，无优雅退出选项（Windows 下进程通常通过窗口消息处理退出，控制台程序通过 `SetConsoleCtrlHandler`）。

---

## 5. 前端实现

### 5.1 表格展示

```html
<table class="w-full">
  <thead>
    <tr>
      <th>Port</th><th>Protocol</th><th>Process</th><th>PID</th><th>Action</th>
    </tr>
  </thead>
  <tbody>
    {#each filtered as entry}
      <tr>
        <td><span class="font-mono">{entry.port}</span></td>
        <td><span class="text-xs">{entry.protocol}</span></td>
        <td>{entry.process_name}</td>
        <td class="font-mono">{entry.pid}</td>
        <td>
          <button onclick={() => killPort(entry.port)} disabled={killing !== null}>
            {killing === entry.port ? "Killing..." : "Kill"}
          </button>
        </td>
      </tr>
    {/each}
  </tbody>
</table>
```

### 5.2 搜索过滤

通过 TopBar 的全局搜索框联动，`getSearchQuery()` 获取搜索条件，支持按端口号、进程名、PID 三字段搜索。

### 5.3 确认对话框

```javascript
async function killPort(port) {
    if (!await showConfirm(`Kill process on port ${port}?`)) return;
    // ...
}
```

---

## 6. 测试

```rust
#[test] fn test_port_entry_serialization()
#[test] fn test_port_entry_sort_key()
#[test] fn test_extract_ss_process_name_normal()
#[test] fn test_extract_ss_process_name_no_match()
#[test] fn test_extract_ss_pid_normal()
#[test] fn test_extract_ss_pid_no_match()
```

测试覆盖序列化、进程名提取（正则解析）、PID 提取等边缘情况。

---

## 7. 关键设计决策

1. **`list_ports` 使用 `Result` 而非 `Vec`**: 因为某些平台上可能命令不可用（如在 Linux 容器中可能无 `/proc/net/tcp`），使用 Result 可以向用户展示友好错误信息

2. **两阶杀进程（Unix）**: SIGTERM → wait → SIGKILL，既保证优雅退出又保证最终能杀死

3. **前端搜索在客户端进行**: 端口数据量通常不大（几百条），前端过滤无需后端交互，减少 IPC 延迟
