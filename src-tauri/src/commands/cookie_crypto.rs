// ==================== Chrome Cookie 解密与密钥获取核心逻辑 ====================
//
// 该模块由 cookie_extractor 拆分出来，仅包含离开浏览器独立的解密逻辑：
// - AES-128-CBC / AES-256-GCM 解密
// - Chrome v10/v11 密钥获取（Keychain / Secret Service / DPAPI）
// - SHA-256 完整性校验
use base64::Engine as _;
#[cfg(target_os = "linux")]
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use std::path::PathBuf;

// ==================== Chrome Cookie 解密核心逻辑 ====================

/// 解密 Chrome 加密的 Cookie 值
///
/// 解密规则（Linux）:
/// - v10/v11 前缀（3字节）
/// - AES-128-CBC 加密，IV = 16个空格（0x20）
/// - 密钥: PBKDF2(SecretService 密码, "saltysalt", 1, 16)
/// - v11 + DB version >= 24: 密文解密后前32字节是 SHA256(host) 完整性校验
pub(super) fn decrypt_cookie_value(
    encrypted: &[u8],
    host_key: &str,
    has_integrity_check: bool,
) -> Result<String, String> {
    if encrypted.len() < 3 {
        return Ok(String::from_utf8_lossy(encrypted).to_string());
    }

    if &encrypted[0..3] == b"v10" || &encrypted[0..3] == b"v11" {
        return decrypt_chrome_v10(&encrypted[3..], host_key, has_integrity_check);
    }

    // 未加密（旧版本或非 Chrome 浏览器）
    Ok(String::from_utf8_lossy(encrypted).to_string())
}

/// Chrome v10+ 解密（AES-256-GCM 或 AES-128-CBC，取决于平台）
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn decrypt_chrome_v10(
    encrypted_data: &[u8],
    _host_key: &str,
    has_integrity_check: bool,
) -> Result<String, String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    let key32 = match get_chrome_encryption_key_v10() {
        Some(k) => k,
        None => return Err("[Failed to get Chrome encryption key]".to_string()),
    };

    let cipher = match Aes256Gcm::new_from_slice(&key32) {
        Ok(c) => c,
        Err(_) => return Err("[Invalid key]".to_string()),
    };

    // v11 格式前面有 16 字节认证标签/header，需要跳过
    let offset = if has_integrity_check { 16 } else { 0 };

    // AES-256-GCM: nonce 12 字节 + tag 至少 16 字节
    if encrypted_data.len() < offset + 12 + 16 {
        return Ok(String::from_utf8_lossy(encrypted_data).to_string());
    }

    let nonce = Nonce::from_slice(&encrypted_data[offset..offset + 12]);
    let ciphertext = &encrypted_data[offset + 12..];

    match cipher.decrypt(nonce, ciphertext) {
        Ok(pt) => Ok(String::from_utf8_lossy(&pt).to_string()),
        Err(_) => Err("[Cookie decryption failed]".to_string()),
    }
}

/// Linux 上 Chrome 使用 AES-128-CBC 加密 Cookie
///
/// 数据格式（v11 前缀已去除）：
///   - bytes [0..16): 认证标签/header（跳过）
///   - bytes [16..32): 随机 IV（16 字节）
///   - bytes [32..): AES-128-CBC 密文
///
/// 密钥通过 Secret Service (D-Bus) 获取后由 PBKDF2 派生
#[cfg(target_os = "linux")]
fn decrypt_chrome_v10(
    encrypted_data: &[u8],
    host_key: &str,
    has_integrity_check: bool,
) -> Result<String, String> {
    let v11_key = match get_chrome_encryption_key_v10() {
        Some(k) => k,
        None => {
            return Ok(decrypt_chrome_v10_fallback(encrypted_data));
        }
    };

    // 需要至少 48 字节: 16(auth) + 16(IV) + 16(最小密文)
    if encrypted_data.len() < 48 {
        return Ok(String::from_utf8_lossy(encrypted_data).to_string());
    }

    // 标准格式: auth_tag[16] + iv[16] + ciphertext
    if let Ok(v) = try_aes_128_cbc(
        &v11_key,
        &encrypted_data[16..32],
        &encrypted_data[32..],
        host_key,
        has_integrity_check,
    ) {
        return Ok(v);
    }
    // 回退: 固定 IV
    let fixed_iv = [0x20u8; 16];
    if let Ok(v) = try_aes_128_cbc(
        &v11_key,
        &fixed_iv,
        encrypted_data,
        host_key,
        has_integrity_check,
    ) {
        return Ok(v);
    }
    Err("Cookie decryption failed".to_string())
}

/// 尝试 AES-128-CBC 解密
#[cfg(target_os = "linux")]
pub(super) fn try_aes_128_cbc(
    key: &[u8; 16],
    iv: &[u8],
    ciphertext: &[u8],
    host_key: &str,
    has_integrity_check: bool,
) -> Result<String, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

    if ciphertext.is_empty() {
        return Err("Empty ciphertext".to_string());
    }

    let dec = Aes128CbcDec::new_from_slices(key, iv)
        .map_err(|e| format!("CBC init error: {}", e))?;

    let mut buf = ciphertext.to_vec();
    let plaintext = dec
        .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf)
        .map_err(|e| format!("Decrypt fail: {}", e))?;

    if has_integrity_check {
        if plaintext.len() <= 32 {
            return Err("Ciphertext too short for integrity check".to_string());
        }
        let integrity_hash = &plaintext[..32];
        let expected_hash = Sha256::digest(host_key.as_bytes());
        if integrity_hash != expected_hash.as_slice() {
            return Err("Integrity check failed".to_string());
        }
        Ok(String::from_utf8_lossy(&plaintext[32..]).to_string())
    } else {
        Ok(String::from_utf8_lossy(plaintext).to_string())
    }
}

/// Linux 回退：尝试用 AES-256-GCM 解密（旧版 Chrome < v127）
#[cfg(target_os = "linux")]
fn decrypt_chrome_v10_fallback(encrypted_data: &[u8]) -> String {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    // 如果缺少 encrypted_key 字段，返回失败
    if encrypted_data.len() < 12 + 16 {
        return String::from_utf8_lossy(encrypted_data).to_string();
    }

    // 尝试从 keyring 获取旧版密钥
    // 使用硬编码的 fallback: 某些旧 Chrome 版本密钥是固定的
    // 更多的 fallback 方式留给 get_chrome_encryption_key_v10 处理
    match get_chrome_encryption_key_v10_fallback() {
        Some(key32) => {
            let cipher = match Aes256Gcm::new_from_slice(&key32) {
                Ok(c) => c,
                Err(_) => return "[Invalid key (fallback)]".to_string(),
            };
            let nonce = Nonce::from_slice(&encrypted_data[0..12]);
            let ciphertext = &encrypted_data[12..];
            match cipher.decrypt(nonce, ciphertext) {
                Ok(pt) => String::from_utf8_lossy(&pt).to_string(),
                Err(_) => "[Fallback decrypt failed]".to_string(),
            }
        }
        None => "[No Chrome encryption key found (Linux)]".to_string(),
    }
}

// ==================== Chrome 加密密钥获取 ====================

/// macOS: 通过 Keychain 获取 Chrome v10+ AES-256-GCM 加密密钥
#[cfg(target_os = "macos")]
fn get_chrome_encryption_key_v10() -> Option<[u8; 32]> {
    use keyring::Entry;
    let entry = Entry::new("Chrome Safe Storage", "Chrome").ok()?;
    let password = entry.get_password().ok()?;
    // macOS 上密码是 base64 编码的 32 字节密钥
    if password.len() == 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(password.as_bytes());
        return Some(key);
    }
    // 也可能是 base64 编码的
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&password) {
        if decoded.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            return Some(key);
        }
    }
    // 回退：尝试直接使用密码的字节
    let bytes = password.as_bytes();
    if bytes.len() >= 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[0..32]);
        return Some(key);
    }
    None
}

/// Linux: 通过 Secret Service 获取 Chrome 加密密钥（v11 AES-128-CBC）
///
/// 尝试多种方式获取，按优先级：
/// 1. secret-tool CLI（无额外依赖）
/// 2. Rust D-Bus 直接访问
/// 3. Python dbus-python（回退）
#[cfg(target_os = "linux")]
fn get_chrome_encryption_key_v10() -> Option<[u8; 16]> {
    // 方法1: 使用 secret-tool CLI
    if let Some(key) = try_secret_tool_v11() {
        return Some(key);
    }

    // 方法2: 使用 Rust D-Bus
    if let Some(key) = try_dbus_rust() {
        return Some(key);
    }

    // 方法3: 使用 Python dbus-python（最后的回退）
    try_dbus_python()
}

/// 通过 secret-tool CLI 获取 Chrome v11 密钥
#[cfg(target_os = "linux")]
fn try_secret_tool_v11() -> Option<[u8; 16]> {
    let output = std::process::Command::new("secret-tool")
        .args([
            "search",
            "--all",
            "xdg:schema",
            "chrome_libsecret_os_crypt_password_v2",
            "application",
            "chrome",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("secret = ") {
            let pwd = value.trim().to_string();
            if !pwd.is_empty() {
                return derive_v11_key_from_str(&pwd);
            }
        }
    }
    None
}

/// 从密码字符串派生 v11 密钥
/// 注意：Chromium 直接将 base64 编码的密钥字符串（不解码）作为 PBKDF2 的 password 输入
#[cfg(target_os = "linux")]
fn derive_v11_key_from_str(secret_str: &str) -> Option<[u8; 16]> {
    let secret_str = secret_str.trim();
    if secret_str.is_empty() {
        return None;
    }

    // 直接使用原始字符串的 ASCII 字节作为 PBKDF2 输入（不要 base64 解码）
    use pbkdf2::pbkdf2_hmac;
    use sha1::Sha1;
    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(secret_str.as_bytes(), b"saltysalt", 1, &mut key);
    Some(key)
}

/// 使用 Rust dbus crate 直接访问 Secret Service
#[cfg(target_os = "linux")]
fn try_dbus_rust() -> Option<[u8; 16]> {
    use dbus::{
        blocking::{BlockingSender, Connection},
        Message,
    };
    use std::collections::HashMap;
    use std::time::Duration;

    let conn = Connection::new_session().ok()?;

    let mut search_attrs = HashMap::new();
    search_attrs.insert(
        "xdg:schema".to_string(),
        "chrome_libsecret_os_crypt_password_v2".to_string(),
    );
    search_attrs.insert("application".to_string(), "chrome".to_string());

    let timeout = Duration::from_secs(3);

    let r = conn
        .send_with_reply_and_block(
            Message::new_method_call(
                "org.freedesktop.secrets",
                "/org/freedesktop/secrets",
                "org.freedesktop.Secret.Service",
                "SearchItems",
            )
            .ok()?
            .append1(search_attrs),
            timeout,
        )
        .ok()?;
    let (items, _prompt): (Vec<dbus::Path<'static>>, dbus::Path<'static>) = r.read2().ok()?;
    let item_path = items.first()?.clone();

    let _ = conn.send_with_reply_and_block(
        Message::new_method_call(
            "org.freedesktop.secrets",
            "/org/freedesktop/secrets",
            "org.freedesktop.Secret.Service",
            "Unlock",
        )
        .ok()?
        .append1(vec![item_path.clone()]),
        timeout,
    );

    let session_path = {
        let r = conn
            .send_with_reply_and_block(
                Message::new_method_call(
                    "org.freedesktop.secrets",
                    "/org/freedesktop/secrets",
                    "org.freedesktop.Secret.Service",
                    "OpenSession",
                )
                .ok()?
                .append2("plain", dbus::arg::Variant(Box::new(String::new()))),
                timeout,
            )
            .ok()?;
        let (result, session): (
            dbus::arg::Variant<Box<dyn dbus::arg::RefArg>>,
            dbus::Path<'static>,
        ) = r.read2().ok()?;
        drop(result);
        session
    };

    let secret_value = {
        let r = conn
            .send_with_reply_and_block(
                Message::new_method_call(
                    "org.freedesktop.secrets",
                    "/org/freedesktop/secrets",
                    "org.freedesktop.Secret.Service",
                    "GetSecrets",
                )
                .ok()?
                .append2(vec![item_path], session_path),
                timeout,
            )
            .ok()?;

        use dbus::arg::ArgType;
        let mut iter = r.iter_init();
        {
            let mut array_iter = iter.recurse(ArgType::Array)?;
            let mut found_value: Option<Vec<u8>> = None;
            while let Some(mut entry_iter) = array_iter.recurse(ArgType::DictEntry) {
                let _key: dbus::Path<'static> = entry_iter.read().ok()?;
                if let Some(mut struct_iter) = entry_iter.recurse(ArgType::Struct) {
                    let _sess: dbus::Path<'static> = struct_iter.read().ok()?;
                    let value: Vec<u8> = struct_iter.read().ok()?;
                    let _content_type: String = struct_iter.read().ok()?;
                    let _parameters: Vec<u8> = struct_iter.read().ok()?;
                    found_value = Some(value);
                }
            }
            found_value?
        }
    };

    derive_v11_key(&secret_value)
}

/// 使用 Python dbus-python 获取 Chrome 加密密钥（最后的回退方案）
#[cfg(target_os = "linux")]
fn try_dbus_python() -> Option<[u8; 16]> {
    let script = r#"
import dbus, sys
bus = dbus.SessionBus()
ss = dbus.Interface(bus.get_object('org.freedesktop.secrets', '/org/freedesktop/secrets'), 'org.freedesktop.Secret.Service')
paths = ss.SearchItems({'xdg:schema': 'chrome_libsecret_os_crypt_password_v2', 'application': 'chrome'})
paths = [p for p in paths if p]
if not paths or not paths[0]:
    sys.exit(1)
obj_path = paths[0][0]
ss.Unlock([obj_path])
_, session = ss.OpenSession('plain', dbus.String('', variant_level=1))
secrets = ss.GetSecrets([obj_path], session)
_, _, value, _ = secrets[obj_path]
sys.stdout.buffer.write(bytes(value))
"#;

    for python_cmd in &["python3", "python"] {
        let output = std::process::Command::new(python_cmd)
            .arg("-c")
            .arg(script)
            .output()
            .ok()?;
        if output.status.success() && !output.stdout.is_empty() {
            return derive_v11_key(&output.stdout);
        }
    }
    None
}

/// 从 Secret Service 获取的原始密码派生 v11 AES-128-CBC 密钥
/// 注意：Chromium 直接将 base64 编码的密钥字符串（不解码）作为 PBKDF2 的 password 输入
#[cfg(target_os = "linux")]
fn derive_v11_key(secret_bytes: &[u8]) -> Option<[u8; 16]> {
    if secret_bytes.is_empty() {
        return None;
    }
    // Secret Service 返回的 secret 是 base64 编码的密码字符串（如 "+kOD1Z0EvL5YjX/9nfyoYA=="）
    let secret_str = std::str::from_utf8(secret_bytes).ok()?.trim();
    if secret_str.is_empty() {
        return None;
    }

    // 直接使用原始 ASCII 字节作为 PBKDF2 输入（不要 base64 解码）
    // PBKDF2-HMAC-SHA1(pwd, "saltysalt", 1, 16) -> v11_key
    use pbkdf2::pbkdf2_hmac;
    use sha1::Sha1;
    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(secret_str.as_bytes(), b"saltysalt", 1, &mut key);
    Some(key)
}

/// Linux 回退方式：尝试通过其他方式获取旧版 Chrome 的 AES-256-GCM 密钥
#[cfg(target_os = "linux")]
fn get_chrome_encryption_key_v10_fallback() -> Option<[u8; 32]> {
    // 方法1: 使用 secret-tool CLI（可能需要手动解锁）
    let output = std::process::Command::new("secret-tool")
        .args(["lookup", "application", "chrome"])
        .output()
        .ok()?;
    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            // secret-tool 返回的是 base64 编码的 32 字节密钥
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&value) {
                if decoded.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&decoded);
                    return Some(key);
                }
            }
        }
    }

    // 方法2: 尝试搜索不同的 schema
    let output = std::process::Command::new("secret-tool")
        .args(["search", "application", "chrome"])
        .output()
        .ok()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(value) = line.strip_prefix("secret = ") {
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value.trim())
                {
                    if decoded.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&decoded);
                        return Some(key);
                    }
                }
            }
        }
    }

    None
}

/// Windows: 通过 DPAPI 解密 Chrome Local State 中的加密密钥
#[cfg(target_os = "windows")]
fn get_chrome_encryption_key_v10() -> Option<[u8; 32]> {
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let local_state_path =
            PathBuf::from(localappdata).join("Google\\Chrome\\User Data\\Local State");

        if let Ok(content) = std::fs::read_to_string(&local_state_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(encoded_key) = json["os_crypt"]["encrypted_key"].as_str() {
                    if let Ok(decoded) =
                        base64::engine::general_purpose::STANDARD.decode(encoded_key)
                    {
                        if decoded.len() >= 5 && &decoded[0..5] == b"DPAPI" {
                            let raw_key = decrypt_windows_raw(&decoded[5..]);
                            if raw_key.len() >= 32 {
                                let mut key = [0u8; 32];
                                key.copy_from_slice(&raw_key[0..32]);
                                return Some(key);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn decrypt_windows_raw(encrypted_data: &[u8]) -> Vec<u8> {
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    unsafe {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: encrypted_data.len() as u32,
            pbData: encrypted_data.as_ptr() as *mut u8,
        };

        let mut data_out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        match CryptUnprotectData(
            &data_in,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut data_out as *mut _,
        ) {
            Ok(()) => {
                if data_out.pbData.is_null() || data_out.cbData == 0 {
                    return encrypted_data.to_vec();
                }
                std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec()
            }
            Err(_) => encrypted_data.to_vec(),
        }
    }
}
