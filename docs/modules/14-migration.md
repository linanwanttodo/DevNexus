# 环境迁移 — 模块设计文档

## 1. 功能概述

环境迁移（Migration）将当前开发环境的配置（环境变量、PATH、软件安装列表等）导出为清单文件，并可在新机器上导入恢复。

**通信链路**:
```
Migration.svelte ──→ invoke("export_migration")        ──→ migration.rs
               ──→ invoke("save_export_file")         ──→ migration.rs
               ──→ invoke("parse_migration_manifest") ──→ migration.rs
               ──→ invoke("load_migration_file")      ──→ migration.rs
               ──→ invoke("import_migration")         ──→ migration.rs
```

---

## 2. 核心命令

| 命令 | 说明 |
|------|------|
| `export_migration` | 按用户选择（`ExportSelection`）导出配置为 JSON 清单 |
| `save_export_file` | 将清单写入目标文件 |
| `parse_migration_manifest` | 解析并校验清单 JSON（非法 JSON / 缺必填字段均报错） |
| `load_migration_file` | 从文件读取并解析清单 |
| `import_migration` | 按清单恢复环境（切换已安装的运行时版本，不自动安装缺失项） |

---

## 3. 数据格式

`MigrationManifest` 为 JSON 清单，字段覆盖环境变量、PATH 项、运行时版本等。解析层（`parse_migration_manifest`）对非法 JSON、缺失必填字段、纯空白输入均返回结构化错误。

---

## 4. 设计要点

- 导入为**非破坏性**：仅切换已安装版本，不安装缺失运行时（`import_note` 文案明确说明）
- 纯函数解析逻辑可单测（4 个解析用例：合法字段、非法 JSON、缺必填、空白）
