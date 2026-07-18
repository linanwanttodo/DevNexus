use serde::{Deserialize, Serialize};

// ==================== 数据模型 ====================

#[derive(Serialize, Deserialize, Clone)]
pub struct MigrationManifest {
    pub meta: MigrationMeta,
    pub environments: Vec<EnvironmentSnapshot>,
    pub versions: Vec<VersionSnapshot>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MigrationMeta {
    pub exported_at: String,
    pub devnexus_version: String,
    pub source_os: String,
    pub hostname: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EnvironmentSnapshot {
    pub name: String,
    pub lang_type: String,
    pub version: String,
    pub path: String,
    pub shell_config: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct VersionSnapshot {
    pub lang_type: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct ExportSelection {
    pub environments: Vec<String>,
    pub versions: Vec<VersionSnapshot>,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub switched: u32,
    pub skipped: u32,
    pub failed: u32,
    pub details: Vec<String>,
}

// ==================== 导出 ====================

#[tauri::command]
pub fn export_migration(selected: ExportSelection) -> Result<String, String> {
    let all_envs = crate::commands::environment::list_environments();
    let selected_envs: Vec<EnvironmentSnapshot> = all_envs
        .into_iter()
        .filter(|e| selected.environments.contains(&e.name))
        .map(|e| EnvironmentSnapshot {
            name: e.name,
            lang_type: e.lang_type,
            version: e.version,
            path: e.path,
            shell_config: e.shell_config,
        })
        .collect();

    let manifest = MigrationManifest {
        meta: MigrationMeta {
            exported_at: chrono::Utc::now().to_rfc3339(),
            devnexus_version: env!("CARGO_PKG_VERSION").to_string(),
            source_os: std::env::consts::OS.to_string(),
            hostname: std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("COMPUTERNAME"))
                .unwrap_or_else(|_| "unknown".to_string()),
        },
        environments: selected_envs,
        versions: selected.versions,
    };

    serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())
}

/// 将所有环境导出并保存到指定路径
#[tauri::command]
pub fn save_export_file(path: String) -> Result<String, String> {
    let all_envs = crate::commands::environment::list_environments();
    let env_names: Vec<String> = all_envs.iter().map(|e| e.name.clone()).collect();

    let json = export_migration(ExportSelection {
        environments: env_names,
        versions: vec![],
    })?;

    std::fs::write(&path, &json).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!(
        "Exported {} environments to {}",
        all_envs.len(),
        path
    ))
}

// ==================== 导入 ====================

/// 解析迁移清单 JSON（供前端预览）
#[tauri::command]
pub fn parse_migration_manifest(json: String) -> Result<MigrationManifest, String> {
    if json.trim().is_empty() {
        return Err("Empty migration file".to_string());
    }
    serde_json::from_str(&json).map_err(|e| format!("Invalid migration JSON: {}", e))
}

/// 从文件路径读取并解析迁移清单
#[tauri::command]
pub fn load_migration_file(path: String) -> Result<MigrationManifest, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    parse_migration_manifest(json)
}

/// 导入迁移清单：可选地对 versions 执行版本切换
#[tauri::command]
pub fn import_migration(
    json: String,
    apply_versions: bool,
    cache: tauri::State<'_, crate::commands::version_manager::VersionCache>,
) -> Result<ImportResult, String> {
    let manifest = parse_migration_manifest(json)?;
    let mut result = ImportResult {
        switched: 0,
        skipped: 0,
        failed: 0,
        details: Vec::new(),
    };

    // 记录环境清单差异（仅信息，不强制安装）
    let local_envs = crate::commands::environment::list_environments();
    for snap in &manifest.environments {
        let local = local_envs.iter().find(|e| e.lang_type == snap.lang_type);
        match local {
            Some(env) => result.details.push(format!(
                "env {}: remote={} local={}",
                snap.lang_type, snap.version, env.version
            )),
            None => result.details.push(format!(
                "env {}: present on source ({}), not found locally",
                snap.lang_type, snap.version
            )),
        }
    }

    if !apply_versions {
        result.skipped = manifest.versions.len() as u32;
        if !manifest.versions.is_empty() {
            result.details.push(format!(
                "skipped {} version switches (apply_versions=false)",
                manifest.versions.len()
            ));
        }
        return Ok(result);
    }

    // 去重：同一 lang_type 只切最后一个选中版本
    let mut by_lang: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for v in &manifest.versions {
        by_lang.insert(v.lang_type.clone(), v.version.clone());
    }

    for (lang_type, version) in by_lang {
        match crate::commands::version_manager::switch_version(
            lang_type.clone(),
            version.clone(),
            cache.clone(),
        ) {
            Ok(msg) => {
                result.switched += 1;
                result
                    .details
                    .push(format!("switched {} → {}: {}", lang_type, version, msg));
            }
            Err(e) => {
                // 本地无对应工具/版本时记为失败，不中断整体导入
                result.failed += 1;
                result
                    .details
                    .push(format!("failed {} → {}: {}", lang_type, version, e));
            }
        }
    }

    Ok(result)
}
