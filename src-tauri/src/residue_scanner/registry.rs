use crate::residue_scanner::ResidueItem;

/// 在 Windows 注册表中扫描应用残留键
/// 搜索 HKCU\Software, HKLM\SOFTWARE, HKLM\SOFTWARE\WOW6432Node
pub fn scan_registry(app_name: &str) -> Vec<ResidueItem> {
    let mut results = Vec::new();
    let name_lower = app_name.to_lowercase();
    let name_key = name_lower.replace([' ', '_', '-'], "");

    // 构建候选名称列表
    let candidates = build_candidates(&name_lower);

    let hives = [
        (
            "HKEY_CURRENT_USER\\Software",
            winreg::enums::HKEY_CURRENT_USER,
            "Software",
        ),
        (
            "HKEY_LOCAL_MACHINE\\SOFTWARE",
            winreg::enums::HKEY_LOCAL_MACHINE,
            "SOFTWARE",
        ),
        (
            "HKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node",
            winreg::enums::HKEY_LOCAL_MACHINE,
            "SOFTWARE\\WOW6432Node",
        ),
    ];

    for (hive_label, hive, subkey) in &hives {
        if let Ok(key) =
            winreg::RegKey::predef(*hive).open_subkey_with_flags(subkey, winreg::enums::KEY_READ)
        {
            for k in key.enum_keys().flatten() {
                let k_lower = k.to_lowercase();
                let k_flat = k_lower.replace([' ', '_', '-'], "");
                for candidate in &candidates {
                    if k_lower.contains(candidate)
                        || k_flat.contains(candidate)
                        || k_flat.contains(&name_key)
                    {
                        let full_path = format!("{}\\{}", hive_label, k);
                        results.push(ResidueItem {
                            path: full_path,
                            size: 0,
                            category: "registry".into(),
                            is_safe_to_delete: true,
                            description: format!("Registry key under {}", hive_label),
                        });
                        break;
                    }
                }
            }
        }
    }

    // 也搜索 Services
    if let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(
            "SYSTEM\\CurrentControlSet\\Services",
            winreg::enums::KEY_READ,
        )
    {
        for svc in key.enum_keys().flatten() {
            let s_lower = svc.to_lowercase();
            for candidate in &candidates {
                if s_lower.contains(candidate) {
                    let full_path = format!("HKLM\\SYSTEM\\CurrentControlSet\\Services\\{}", svc);
                    results.push(ResidueItem {
                        path: full_path,
                        size: 0,
                        category: "service".into(),
                        is_safe_to_delete: true,
                        description: "Windows service registry entry".into(),
                    });
                    break;
                }
            }
        }
    }

    // 搜索 Uninstall 键下的 DisplayName 匹配 （已安装程序中匹配）
    let uninstall_paths = [
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
    ];
    for unsub in &uninstall_paths {
        if let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(unsub, winreg::enums::KEY_READ)
        {
            for subk in key.enum_keys().flatten() {
                if let Ok(app_key) = key.open_subkey_with_flags(&subk, winreg::enums::KEY_READ) {
                    if let Ok(display_name) = app_key.get_value::<String, _>("DisplayName") {
                        let dn_lower = display_name.to_lowercase();
                        if dn_lower.contains(&name_lower) {
                            let full_path = format!("HKLM\\{unsub}\\{subk}");
                            results.push(ResidueItem {
                                path: full_path,
                                size: 0,
                                category: "registry".into(),
                                is_safe_to_delete: false, // 卸载条目，不由我们直接删
                                description: "Program uninstall registry entry".into(),
                            });
                        }
                    }
                }
            }
        }
    }

    results
}

fn build_candidates(name: &str) -> Vec<String> {
    let mut candidates = vec![name.to_string()];
    for sep in &[' ', '-', '_', '.'] {
        for part in name.split(*sep) {
            let part = part.trim();
            if part.len() >= 3 && !candidates.contains(&part.to_string()) {
                candidates.push(part.to_string());
            }
        }
    }
    // 特殊映射
    match name {
        n if n.contains("microsoft") || n.contains("ms") => {
            candidates.push("microsoft".into());
        }
        n if n.contains("google") => {
            candidates.push("google".into());
        }
        n if n.contains("jetbrains") || n.contains("intellij") => {
            candidates.push("jetbrains".into());
        }
        _ => {}
    }
    candidates
}
