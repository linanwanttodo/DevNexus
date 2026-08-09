# 容器管理 — 模块设计文档

## 1. 功能概述

容器管理器（Container Manager）提供 Docker/Podman 容器、镜像、卷、网络与 Compose 的一站式管理，覆盖常见运维操作。

**通信链路**:
```
ContainerManager.vue ──→ invoke("list_containers")  ──→ container.rs
                    ──→ invoke("container_action")  ──→ container.rs
                    ──→ invoke("list_images")       ──→ container.rs
                    ──→ invoke("list_volumes")      ──→ container.rs
                    ──→ invoke("list_networks")     ──→ container.rs
                    ──→ invoke("compose_up")        ──→ container.rs
```

---

## 2. 核心命令

| 命令 | 说明 |
|------|------|
| `check_docker` | 检测 docker 是否安装、版本与运行状态 |
| `list_containers` / `container_action` | 容器列表；start/stop/restart/pause/unpause/kill/rm/rename |
| `get_container_logs` / `exec_in_container` | 容器日志；进入容器执行命令 |
| `list_images` / `pull_image` / `remove_image` / `build_image` / `tag_image` / `push_image` | 镜像全生命周期 |
| `list_volumes` / `volume_action` | 卷列表与删除 |
| `list_networks` / `network_action` | 网络列表与删除 |
| `compose_up` / `compose_down` / `compose_ps` / `compose_logs` | Compose 项目管理 |

---

## 3. 安全设计

- 所有 docker 命令经 `utils::exec` 统一执行，120s 超时（防 docker 卡死阻塞）
- 容器 action 白名单校验（`ALLOWED_ACTIONS`），容器名/命令参数拒绝 shell 元字符与 `-` 选项注入
- 输出 JSON 行解析（`parse_json_lines`）容错跳过非法行

---

## 4. 前端结构

`ContainerManager.vue` 按 tab 拆分为 5 个表格组件（`src/components/containers/` 下）：
`ContainersTab` / `ImagesTab` / `VolumesTab` / `NetworksTab` / `ComposeTab`，弹窗统一走 `ContainerDialog.vue`（配置对象驱动）。
