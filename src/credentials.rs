use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use reqwest::header::{CONTENT_TYPE, USER_AGENT as USER_AGENT_HEADER};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::env_file::{env_value, parse_env};

pub const REGISTER_URL: &str = "https://log.snssdk.com/service/2/device_register/";
pub const SETTINGS_URL: &str = "https://is.snssdk.com/service/settings/v3/";
pub const AID: u32 = 401734;
pub const APP_NAME: &str = "oime";
pub const VERSION_CODE: u32 = 100102018;
pub const VERSION_NAME: &str = "1.1.2";
pub const CHANNEL: &str = "official";
pub const PACKAGE: &str = "com.bytedance.android.doubaoime";
pub const USER_AGENT: &str = "com.bytedance.android.doubaoime/100102018 (Linux; U; Android 16; en_US; Pixel 7 Pro; Build/BP2A.250605.031.A2; Cronet/TTNetVersion:94cf429a 2025-11-17 QuicVersion:1f89f732 2025-05-08)";
const DEVICE_ID_KEY: &str = "device_id";
const INSTALL_ID_KEY: &str = "install_id";
const CDID_KEY: &str = "cdid";
const OPENUDID_KEY: &str = "openudid";
const CLIENTUDID_KEY: &str = "clientudid";
const TOKEN_KEY: &str = "token";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CachedCredentials {
    pub device_id: String,
    pub install_id: String,
    pub cdid: String,
    pub openudid: String,
    pub clientudid: String,
    pub token: String,
}

impl CachedCredentials {
    pub fn load(path: &Path) -> Result<Self> {
        let data = fs::read_to_string(path)
            .with_context(|| format!("failed to read env file {}", path.display()))?;
        let env = parse_env(&data);
        let get = |key: &str| env_value(&env, key).unwrap_or_default();

        Ok(Self {
            device_id: get(DEVICE_ID_KEY),
            install_id: get(INSTALL_ID_KEY),
            cdid: get(CDID_KEY),
            openudid: get(OPENUDID_KEY),
            clientudid: get(CLIENTUDID_KEY),
            token: get(TOKEN_KEY),
        })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(missing_env_error(path));
        }
        let existing = fs::read_to_string(path)
            .with_context(|| format!("failed to read env file {}", path.display()))?;
        let data = update_env_credentials(&existing, self);
        fs::write(path, data)
            .with_context(|| format!("failed to write env file {}", path.display()))
    }
}

pub fn default_env_path() -> PathBuf {
    PathBuf::from(".env")
}

pub fn is_jwt_expired(token: &str, margin_seconds: u64) -> bool {
    let Some(payload) = token.split('.').nth(1) else {
        return false;
    };
    let Ok(decoded) = URL_SAFE_NO_PAD.decode(payload) else {
        return false;
    };
    let Ok(json) = serde_json::from_slice::<Value>(&decoded) else {
        return false;
    };
    let Some(exp) = json.get("exp").and_then(Value::as_u64) else {
        return false;
    };
    now_seconds() >= exp.saturating_sub(margin_seconds)
}

pub async fn ensure_credentials(
    client: &reqwest::Client,
    path: &Path,
    reset: bool,
) -> Result<CachedCredentials> {
    if !path.exists() {
        return Err(missing_env_error(path));
    }

    if reset {
        let existing = fs::read_to_string(path).unwrap_or_default();
        fs::write(path, remove_env_credentials(&existing))
            .with_context(|| format!("failed to reset env credentials {}", path.display()))?;
    }

    if let Ok(mut cached) = CachedCredentials::load(path) {
        if !cached.device_id.is_empty()
            && !cached.token.is_empty()
            && !is_jwt_expired(&cached.token, 60)
        {
            return Ok(cached);
        }
        if !cached.device_id.is_empty() {
            cached.token = fetch_asr_token(client, &cached.device_id, &cached.cdid).await?;
            cached.save(path)?;
            return Ok(cached);
        }
    }

    let mut fresh = register_device(client).await?;
    fresh.token = fetch_asr_token(client, &fresh.device_id, &fresh.cdid).await?;
    fresh.save(path)?;
    Ok(fresh)
}

fn missing_env_error(path: &Path) -> anyhow::Error {
    let custom_path_note = if path == Path::new(".env") {
        String::new()
    } else {
        format!(
            " If you use --env-path, copy the template to {} instead.",
            path.display()
        )
    };

    anyhow!(
        "Missing .env file at {}. Run `cp .env.example .env` first, then start SeedRelay again.{} Do not edit device_id, install_id, cdid, openudid, clientudid, or token manually; SeedRelay updates them automatically after registration or token refresh.",
        path.display(),
        custom_path_note
    )
}

fn update_env_credentials(existing: &str, credentials: &CachedCredentials) -> String {
    let mut preserved = remove_env_credentials(existing);
    if !preserved.is_empty() && !preserved.ends_with('\n') {
        preserved.push('\n');
    }
    preserved.push_str(DEVICE_ID_KEY);
    preserved.push('=');
    preserved.push_str(&credentials.device_id);
    preserved.push('\n');
    preserved.push_str(INSTALL_ID_KEY);
    preserved.push('=');
    preserved.push_str(&credentials.install_id);
    preserved.push('\n');
    preserved.push_str(CDID_KEY);
    preserved.push('=');
    preserved.push_str(&credentials.cdid);
    preserved.push('\n');
    preserved.push_str(OPENUDID_KEY);
    preserved.push('=');
    preserved.push_str(&credentials.openudid);
    preserved.push('\n');
    preserved.push_str(CLIENTUDID_KEY);
    preserved.push('=');
    preserved.push_str(&credentials.clientudid);
    preserved.push('\n');
    preserved.push_str(TOKEN_KEY);
    preserved.push('=');
    preserved.push_str(&credentials.token);
    preserved.push('\n');
    preserved
}

fn remove_env_credentials(existing: &str) -> String {
    existing
        .lines()
        .filter(|line| {
            let key = line
                .trim_start()
                .split_once('=')
                .map(|(key, _)| key.trim())
                .unwrap_or("");
            !matches!(
                key,
                DEVICE_ID_KEY
                    | INSTALL_ID_KEY
                    | CDID_KEY
                    | OPENUDID_KEY
                    | CLIENTUDID_KEY
                    | TOKEN_KEY
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn register_device(client: &reqwest::Client) -> Result<CachedCredentials> {
    let cdid = Uuid::new_v4().to_string();
    let openudid = random_openudid();
    let clientudid = Uuid::new_v4().to_string();
    let now_ms = now_millis();

    let params = vec![
        ("device_platform", "android".to_string()),
        ("os", "android".to_string()),
        ("ssmix", "a".to_string()),
        ("_rticket", now_ms.to_string()),
        ("cdid", cdid.clone()),
        ("channel", CHANNEL.to_string()),
        ("aid", AID.to_string()),
        ("app_name", APP_NAME.to_string()),
        ("version_code", VERSION_CODE.to_string()),
        ("version_name", VERSION_NAME.to_string()),
        ("manifest_version_code", VERSION_CODE.to_string()),
        ("update_version_code", VERSION_CODE.to_string()),
        ("resolution", "1080*2400".to_string()),
        ("dpi", "420".to_string()),
        ("device_type", "Pixel 7 Pro".to_string()),
        ("device_brand", "google".to_string()),
        ("language", "zh".to_string()),
        ("os_api", "34".to_string()),
        ("os_version", "16".to_string()),
        ("ac", "wifi".to_string()),
    ];

    let body = serde_json::json!({
        "magic_tag": "ss_app_log",
        "header": {
            "device_id": 0,
            "install_id": 0,
            "aid": AID,
            "app_name": APP_NAME,
            "version_code": VERSION_CODE,
            "version_name": VERSION_NAME,
            "manifest_version_code": VERSION_CODE,
            "update_version_code": VERSION_CODE,
            "channel": CHANNEL,
            "package": PACKAGE,
            "device_platform": "android",
            "os": "android",
            "os_api": "34",
            "os_version": "16",
            "device_type": "Pixel 7 Pro",
            "device_brand": "google",
            "device_model": "Pixel 7 Pro",
            "resolution": "1080*2400",
            "dpi": "420",
            "language": "zh",
            "timezone": 8,
            "access": "wifi",
            "rom": "UP1A.231005.007",
            "rom_version": "UP1A.231005.007",
            "openudid": openudid,
            "clientudid": clientudid,
            "cdid": cdid,
            "region": "CN",
            "tz_name": "Asia/Shanghai",
            "tz_offset": 28800,
            "sim_region": "cn",
            "carrier_region": "cn",
            "cpu_abi": "arm64-v8a",
            "build_serial": "unknown",
            "not_request_sender": 0,
            "sig_hash": "",
            "google_aid": "",
            "mc": "",
            "serial_number": ""
        },
        "_gen_time": now_ms
    });

    let response = client
        .post(REGISTER_URL)
        .query(&params)
        .header(USER_AGENT_HEADER, USER_AGENT)
        .json(&body)
        .send()
        .await
        .context("device registration request failed")?;

    let status = response.status();
    let payload: Value = response.json().await.context("invalid register JSON")?;
    if !status.is_success() {
        return Err(anyhow!(
            "device registration failed: HTTP {status} {payload}"
        ));
    }

    let device_id = json_string_or_number(&payload, "device_id_str", "device_id")
        .ok_or_else(|| anyhow!("device registration response missing device_id: {payload}"))?;
    let install_id =
        json_string_or_number(&payload, "install_id_str", "install_id").unwrap_or_default();

    Ok(CachedCredentials {
        device_id,
        install_id,
        cdid,
        openudid,
        clientudid,
        token: String::new(),
    })
}

async fn fetch_asr_token(client: &reqwest::Client, device_id: &str, cdid: &str) -> Result<String> {
    let body = "body=null";
    let x_ss_stub = format!("{:x}", md5::compute(body.as_bytes())).to_uppercase();
    let params = vec![
        ("device_platform", "android".to_string()),
        ("os", "android".to_string()),
        ("ssmix", "a".to_string()),
        ("_rticket", now_millis().to_string()),
        ("cdid", cdid.to_string()),
        ("channel", CHANNEL.to_string()),
        ("aid", AID.to_string()),
        ("app_name", APP_NAME.to_string()),
        ("version_code", VERSION_CODE.to_string()),
        ("version_name", VERSION_NAME.to_string()),
        ("device_id", device_id.to_string()),
    ];

    let response = client
        .post(SETTINGS_URL)
        .query(&params)
        .header(USER_AGENT_HEADER, USER_AGENT)
        .header(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .header("x-ss-stub", x_ss_stub)
        .body(body.to_string())
        .send()
        .await
        .context("ASR token request failed")?;

    let status = response.status();
    let payload: Value = response.json().await.context("invalid settings JSON")?;
    if !status.is_success() {
        return Err(anyhow!("ASR token request failed: HTTP {status} {payload}"));
    }

    payload
        .pointer("/data/settings/asr_config/app_key")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("settings response missing asr_config.app_key: {payload}"))
}

fn json_string_or_number(payload: &Value, string_key: &str, number_key: &str) -> Option<String> {
    payload
        .get(string_key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && *value != "0")
        .map(ToString::to_string)
        .or_else(|| {
            payload
                .get(number_key)
                .and_then(Value::as_u64)
                .filter(|value| *value != 0)
                .map(|value| value.to_string())
        })
}

fn random_openudid() -> String {
    let mut bytes = [0u8; 8];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as u64
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
}
