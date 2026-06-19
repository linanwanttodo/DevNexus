use serde::{Deserialize, Serialize};

// ==================== 数据模型 ====================

#[derive(Serialize, Deserialize)]
pub struct MigrationManifest {
    pub meta: MigrationMeta,
    pub environments: Vec<EnvironmentSnapshot>,
    pub versions: Vec<VersionSnapshot>,
}

#[derive(Serialize, Deserialize)]
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
