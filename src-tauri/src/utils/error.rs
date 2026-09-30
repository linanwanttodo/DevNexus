use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

/// 统一结构化错误：code 为稳定错误码（前端按 errors.<code> 翻译），
/// params 为 i18n 插值参数，detail 为原始细节（仅日志与详情展示，不直接面向用户）。
/// TODO(container 模块迁移后移除)：首个命令模块接入前暂无使用方
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct DevNexusError {
    pub code: String,
    pub params: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[allow(dead_code)]
impl DevNexusError {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            params: HashMap::new(),
            detail: None,
        }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn invalid_input(detail: impl Into<String>) -> Self {
        Self::new("COMMON_INVALID_INPUT").detail(detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new("COMMON_NOT_FOUND").detail(detail)
    }

    pub fn permission(detail: impl Into<String>) -> Self {
        Self::new("COMMON_PERMISSION_DENIED").detail(detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new("COMMON_INTERNAL").detail(detail)
    }

    pub fn cancelled() -> Self {
        Self::new("COMMON_CANCELLED")
    }
}

impl fmt::Display for DevNexusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.detail, self.params.get("op")) {
            (Some(d), _) => write!(f, "{}: {}", self.code, d),
            (None, Some(op)) => write!(f, "{} ({})", self.code, op),
            _ => write!(f, "{}", self.code),
        }
    }
}

impl std::error::Error for DevNexusError {}

impl From<String> for DevNexusError {
    fn from(s: String) -> Self {
        DevNexusError::internal(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialized_shape() {
        let e = DevNexusError::new("DOCKER_NOT_RUNNING").detail("daemon down");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert_eq!(v["code"], "DOCKER_NOT_RUNNING");
        assert_eq!(v["detail"], "daemon down");
        assert!(v["params"].is_object());
    }

    #[test]
    fn test_detail_skipped_when_none() {
        let e = DevNexusError::new("COMMON_CANCELLED");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert!(v.get("detail").is_none());
        assert_eq!(v["code"], "COMMON_CANCELLED");
    }

    #[test]
    fn test_params_interpolation_input() {
        let e = DevNexusError::new("DOCKER_INVALID_ACTION").param("action", "rename");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert_eq!(v["params"]["action"], "rename");
    }

    #[test]
    fn test_from_string_maps_to_internal() {
        let e: DevNexusError = String::from("boom").into();
        assert_eq!(e.code, "COMMON_INTERNAL");
        assert_eq!(e.detail.as_deref(), Some("boom"));
    }

    #[test]
    fn test_display_includes_detail() {
        let e = DevNexusError::new("X").detail("raw");
        assert_eq!(e.to_string(), "X: raw");
    }
}
