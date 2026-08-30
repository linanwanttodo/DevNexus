# DevNexus 架构改进计划

## 执行摘要

基于全面的架构检测，本项目在代码组织、分层设计和可维护性方面存在系统性问题。本计划提出分三阶段的改进方案，预计需要 6-8 周完成核心重构。

---

## 🔴 核心问题概览

### 1. 巨型文件阻碍维护 (High)
- **software.rs**: 2,267 行 - 最严重
- **tuning.rs**: 1,827 行 - 已有拆分计划
- **version_manager.rs**: 1,322 行
- **ssh/session.rs**: 1,214 行

### 2. 缺乏代码分层 (High)
- Tauri commands 直接包含全部业务逻辑
- 无法复用业务逻辑进行单元测试
- 前端迁移成本高

### 3. 公共 API 暴露过度 (High)
- software.rs: 24 个 pub items
- tuning.rs: 40 个 pub items
- 内部辅助函数被意外依赖

### 4. 跨模块依赖混乱 (Medium)
- commands 之间相互引用
- 循环依赖风险
- 编译耦合度高

### 5. 平台代码隔离不足 (Medium)
- 23 个文件使用 `#[cfg]`，共 218 处
- 未形成统一的平台抽象层
- 新增平台支持困难

---

## 📋 改进路线图

### 阶段 1: 紧急止血（1-2 周）

**目标**: 解决最影响日常开发的痛点

#### 任务 1.1: 拆分 software.rs  已有计划
- **工时**: 6 小时
- **参考**: `docs/refactoring/software-module-plan.md`
- **产出**:
  ```
  commands/software/
  ├── mod.rs
  ├── installer.rs        (~400 lines)
  ├── uninstaller.rs      (~350 lines)
  ├── scanner.rs          (~500 lines)
  ├── version_manager.rs  (~300 lines)
  ├── residue_cleaner.rs  (~400 lines)
  └── process_manager.rs  (~100 lines)
  ```

#### 任务 1.2: 拆分 version_manager.rs
- **工时**: 4 小时
- **产出**:
  ```
  commands/version/
  ├── mod.rs
  ├── detection.rs       # 版本发现逻辑
  ├── github_fetcher.rs  # GitHub releases API
  ├── dist_fetchers.rs   # Node.js/Go 官方源
  └── cache.rs           # 缓存管理
  ```

#### 任务 1.3: 收敛公共 API
- **工时**: 4 小时
- **行动**:
  1. 审查所有 `pub` items
  2. 将内部函数改为 `pub(crate)` 或私有
  3. 为必须公开的 API 添加文档
- **工具**: `cargo doc --no-deps` 检查公开 API

#### 任务 1.4: 补充关键测试
- **工时**: 4 小时
- **目标模块**:
  - `island_bridge.rs` (890 行，无测试)
  - `container.rs` (722 行，测试不足)

---

### 阶段 2: 架构重构（2-4 周）

**目标**: 建立清晰的分层架构和平台抽象

#### 任务 2.1: 引入三层架构
- **工时**: 16 小时
- **新结构**:
  ```
  src/
  ├── presentation/         # Tauri IPC 层（薄包装）
  │   └── commands/         # 仅做参数验证 + 调用 services
  ├── application/          # 业务逻辑层
  │   ├── software_service.rs
  │   ├── tuning_service.rs
  │   ├── ssh_service.rs
  │   └── ...
  ├── domain/               # 核心领域模型
  │   ├── models/
  │   │   ├── software.rs
  │   │   ├── user.rs
  │   │   └── ...
  │   └── repositories/
  └── infrastructure/       # 基础设施适配
      ├── http_client.rs
      ├── db.rs
      └── platform/
  ```

- **示例重构**:
  ```rust
  //  Before
  #[tauri::command]
  pub async fn list_software() -> Result<Vec<Software>, String> {
      // 200+ 行扫描逻辑...
  }

  //  After
  #[tauri::command]
  pub async fn list_software(
      state: State<AppState>
  ) -> Result<Vec<Software>, AppError> {
      let service = SoftwareService::new(&state);
      service.list_installed_apps().await
  }
  ```

#### 任务 2.2: 拆分 tuning.rs 为平台专属模块
- **工时**: 8 小时
- **参考**: `docs/refactoring/tuning-module-plan.md`
- **产出**:
  ```
  commands/tuning/
  ├── mod.rs
  ├── disk_cleanup.rs      (~400 lines)
  ├── exclusions.rs        (~150 lines)
  ├── linux_tuning.rs      (~600 lines)
  └── windows_tuning.rs    (~300 lines)
  ```

#### 任务 2.3: 创建平台抽象层
- **工时**: 12 小时
- **结构**:
  ```
  src/platform/
  ├── mod.rs              # trait 定义
  ├── linux.rs            # #[cfg(target_os = "linux")]
  ├── macos.rs            # #[cfg(target_os = "macos")]
  ├── windows.rs          # #[cfg(target_os = "windows")]
  └── common.rs           # 共享逻辑
  ```

- **接口示例**:
  ```rust
  pub trait PlatformOps {
      fn get_data_dir(&self) -> PathBuf;
      fn get_user_home(&self) -> PathBuf;
      fn set_window_transparent(&self, window: &Window) -> Result<()>;
      fn get_package_managers(&self) -> Vec<PackageManager>;
  }
  ```

#### 任务 2.4: 统一错误处理体系
- **工时**: 8 小时
- **行动**:
  1. 扩展 `DevNexusError` 枚举
  2. 实现 `From` 转换
  3. 逐步迁移所有 commands

- **示例**:
  ```rust
  pub enum DevNexusError {
      #[error("invalid input: {0}")]
      InvalidInput(String),
      #[error("not found: {0}")]
      NotFound(String),
      #[error("permission denied: {0}")]
      Permission(String),
      #[error("network error: {source}")]
      Network { source: reqwest::Error },
      #[error("io error: {source}")]
      Io { source: std::io::Error },
      #[error("platform unsupported: {0}")]
      PlatformUnsupported(String),
  }

  impl From<std::io::Error> for DevNexusError { ... }
  impl From<reqwest::Error> for DevNexusError { ... }
  ```

---

### 阶段 3: 质量提升（持续）

**目标**: 完善测试、文档和命名规范

#### 任务 3.1: 补充集成测试
- **工时**: 12 小时
- **结构**:
  ```
  tests/
  ├── integration/
  │   ├── software_install_test.rs
  │   ├── ssh_connection_test.rs
  │   └── api_hub_e2e_test.rs
  └── fixtures/
      └── mock_responses.json
  ```

#### 任务 3.2: 编写架构文档
- **工时**: 6 小时
- **产出**:
  - `ARCHITECTURE.md` - 系统架构图 + 模块职责
  - `CONTRIBUTING.md` - 开发规范
  - `docs/adr/` - 架构决策记录

#### 任务 3.3: 重构 lib.rs 启动逻辑
- **工时**: 6 小时
- **结构**:
  ```
  src/bootstrap/
  ├── mod.rs
  ├── tray_setup.rs
  ├── island_init.rs
  ├── watchdog.rs
  └── platform_init.rs
  ```

#### 任务 3.4: 优化模块命名
- **工时**: 4 小时
- **调整**:
  - `local_files.rs` → `file_proxy.rs`
  - `window_factory.rs` → `main_window.rs`
  - `known_paths.rs` → `paths_database.rs`

---

## 📊 预期收益

### 量化指标

| 指标 | 当前 | 目标 | 改进 |
|------|------|------|------|
| 最大文件行数 | 2,267 | <500 | -78% |
| 测试覆盖率 | ~40% | 60% | +20% |
| pub items 数量 | ~200 | <100 | -50% |
| 平均编译时间 | ~15s | ~8s | -47% |
| 代码重复率 | ~15% | <5% | -67% |

### 定性收益

1. **可维护性**: 新人可在 1 天内理解模块结构
2. **可测试性**: 业务逻辑可独立于 Tauri 进行测试
3. **可扩展性**: 新增平台只需实现 trait，无需修改核心逻辑
4. **可靠性**: 清晰的错误类型便于定位和恢复

---

## ⚠️ 风险与缓解

### 风险 1: 重构引入回归 bug
- **缓解**: 
  - 每个任务完成后运行完整测试套件
  - 保持向后兼容的公共 API
  - 采用渐进式重构，避免大爆炸式改动

### 风险 2: 工期超出预期
- **缓解**:
  - 优先完成阶段 1 的高优先级任务
  - 阶段 2 可按需拆分，不必一次性完成
  - 保留现有功能作为 fallback

### 风险 3: 团队学习曲线
- **缓解**:
  - 提供详细的迁移指南
  - 举办架构分享会
  - 保留旧代码作为参考

---

## 🎯 成功标准

### 阶段 1 完成标志
- [ ] software.rs 拆分为 6 个子模块
- [ ] version_manager.rs 拆分为 4 个子模块
- [ ] 所有内部函数标记为私有
- [ ] 关键模块测试覆盖率达到 50%

### 阶段 2 完成标志
- [ ] 三层架构落地，commands 仅做薄包装
- [ ] tuning.rs 按平台拆分
- [ ] 平台抽象层 trait 定义完成
- [ ] 统一错误类型被所有 commands 采用

### 阶段 3 完成标志
- [ ] 集成测试覆盖核心流程
- [ ] ARCHITECTURE.md 和 CONTRIBUTING.md 完成
- [ ] lib.rs 启动逻辑提取到 bootstrap 模块
- [ ] 所有模块命名符合规范

---

## 📅 时间规划

```
Week 1-2:  阶段 1 - 紧急止血
  Week 1:  software.rs + version_manager.rs 拆分
  Week 2:  公共 API 收敛 + 测试补充

Week 3-6:  阶段 2 - 架构重构
  Week 3:  三层架构设计 + POC
  Week 4:  迁移核心模块到 service 层
  Week 5:  平台抽象层实现
  Week 6:  统一错误处理 + tuning.rs 拆分

Week 7-8:  阶段 3 - 质量提升
  Week 7:  集成测试 + 架构文档
  Week 8:  启动逻辑重构 + 命名优化
```

---

## 🔗 相关文档

- [Software Module Split Plan](refactoring/software-module-plan.md)
- [Software Implementation Guide](refactoring/software-module-implementation.md)
- [Tuning Module Split Plan](refactoring/tuning-module-plan.md)
- [Logging Migration Plan](refactoring/logging-migration-plan.md)
- [Dependency Management Guide](refactoring/dependency-management.md)

---

## 👥 责任人分配

| 任务 | 负责人 | 预计开始 | 预计完成 |
|------|--------|---------|---------|
| 阶段 1 | TBD | Week 1 | Week 2 |
| 阶段 2 | TBD | Week 3 | Week 6 |
| 阶段 3 | TBD | Week 7 | Week 8 |

---

**最后更新**: 2026-08-30
**版本**: v1.0
**状态**: 待审批
