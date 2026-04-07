//! Minimal S3 V4 signature + PUT/GET/HEAD for Garage-compatible storage.

use crate::config::SyncConfig;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

const REGION: &str = "garage";
const SERVICE: &str = "s3";

/// Upload `body` to `{bucket}/{key}`.
pub async fn s3_put(cfg: &SyncConfig, key: &str, body: &[u8]) -> Result<(), String> {
    validate_config(cfg)?;

    let now = chrono::Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

    let content_hash = hex_sha256(body);
    let url = format!("{}/{}/{}", cfg.endpoint.trim_end_matches('/'), cfg.bucket, key);
    let host = extract_host(&cfg.endpoint)?;

    let headers = vec![
        ("host", host.as_str()),
        ("x-amz-content-sha256", content_hash.as_str()),
        ("x-amz-date", amz_date.as_str()),
    ];

    let auth = sign_v4(
        "PUT",
        &format!("/{}/{}", cfg.bucket, key),
        "",
        &headers,
        &content_hash,
        &date_stamp,
        &amz_date,
        &cfg.access_key,
        &cfg.secret_key,
    );

    let resp = reqwest::Client::new()
        .put(&url)
        .header("host", &host)
        .header("x-amz-content-sha256", &content_hash)
        .header("x-amz-date", &amz_date)
        .header("authorization", &auth)
        .header("content-length", body.len().to_string())
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| format!("S3 PUT 请求失败: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("S3 PUT 失败 ({}): {}", status, text))
    }
}

/// Download `{bucket}/{key}` and return body bytes.
pub async fn s3_get(cfg: &SyncConfig, key: &str) -> Result<Vec<u8>, String> {
    validate_config(cfg)?;

    let now = chrono::Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

    let content_hash = hex_sha256(b""); // empty body for GET
    let url = format!("{}/{}/{}", cfg.endpoint.trim_end_matches('/'), cfg.bucket, key);
    let host = extract_host(&cfg.endpoint)?;

    let headers = vec![
        ("host", host.as_str()),
        ("x-amz-content-sha256", content_hash.as_str()),
        ("x-amz-date", amz_date.as_str()),
    ];

    let auth = sign_v4(
        "GET",
        &format!("/{}/{}", cfg.bucket, key),
        "",
        &headers,
        &content_hash,
        &date_stamp,
        &amz_date,
        &cfg.access_key,
        &cfg.secret_key,
    );

    let resp = reqwest::Client::new()
        .get(&url)
        .header("host", &host)
        .header("x-amz-content-sha256", &content_hash)
        .header("x-amz-date", &amz_date)
        .header("authorization", &auth)
        .send()
        .await
        .map_err(|e| format!("S3 GET 请求失败: {}", e))?;

    if resp.status().is_success() {
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("读取 S3 响应失败: {}", e))
    } else if resp.status().as_u16() == 404 {
        Err("云端不存在该文件，请先导出".to_string())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("S3 GET 失败 ({}): {}", status, text))
    }
}

/// Check if `{bucket}/{key}` exists (HEAD request). Returns true/false.
pub async fn s3_head(cfg: &SyncConfig, key: &str) -> Result<bool, String> {
    validate_config(cfg)?;

    let now = chrono::Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

    let content_hash = hex_sha256(b"");
    let url = format!("{}/{}/{}", cfg.endpoint.trim_end_matches('/'), cfg.bucket, key);
    let host = extract_host(&cfg.endpoint)?;

    let headers = vec![
        ("host", host.as_str()),
        ("x-amz-content-sha256", content_hash.as_str()),
        ("x-amz-date", amz_date.as_str()),
    ];

    let auth = sign_v4(
        "HEAD",
        &format!("/{}/{}", cfg.bucket, key),
        "",
        &headers,
        &content_hash,
        &date_stamp,
        &amz_date,
        &cfg.access_key,
        &cfg.secret_key,
    );

    let resp = reqwest::Client::new()
        .head(&url)
        .header("host", &host)
        .header("x-amz-content-sha256", &content_hash)
        .header("x-amz-date", &amz_date)
        .header("authorization", &auth)
        .send()
        .await
        .map_err(|e| format!("S3 HEAD 请求失败: {}", e))?;

    match resp.status().as_u16() {
        200 => Ok(true),
        404 => Ok(false),
        403 => Err("认证失败，请检查 Access Key 和 Secret Key".to_string()),
        status => Err(format!("S3 HEAD 失败 ({})", status)),
    }
}

// ── S3 V4 Signature ─────────────────────────────────────────────

fn sign_v4(
    method: &str,
    path: &str,
    query: &str,
    headers: &[(&str, &str)],
    payload_hash: &str,
    date_stamp: &str,
    amz_date: &str,
    access_key: &str,
    secret_key: &str,
) -> String {
    // Canonical headers (must be sorted by header name)
    let mut sorted_headers = headers.to_vec();
    sorted_headers.sort_by_key(|(k, _)| k.to_lowercase());

    let canonical_headers: String = sorted_headers
        .iter()
        .map(|(k, v)| format!("{}:{}\n", k.to_lowercase(), v.trim()))
        .collect();
    let signed_headers: String = sorted_headers
        .iter()
        .map(|(k, _)| k.to_lowercase())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, path, query, canonical_headers, signed_headers, payload_hash,
    );

    let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, REGION, SERVICE);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        credential_scope,
        hex_sha256(canonical_request.as_bytes()),
    );

    let signing_key = derive_signing_key(secret_key, date_stamp);
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        access_key, credential_scope, signed_headers, signature,
    )
}

fn derive_signing_key(secret_key: &str, date_stamp: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, REGION.as_bytes());
    let k_service = hmac_sha256(&k_region, SERVICE.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length is always valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn extract_host(endpoint: &str) -> Result<String, String> {
    let url: url::Url = endpoint.parse().map_err(|e| format!("无效的 Endpoint URL: {}", e))?;
    let host = url.host_str().ok_or("Endpoint 缺少 host")?.to_string();
    match url.port() {
        Some(port) => Ok(format!("{}:{}", host, port)),
        None => Ok(host),
    }
}

fn validate_config(cfg: &SyncConfig) -> Result<(), String> {
    if cfg.endpoint.is_empty() {
        return Err("未配置 S3 Endpoint".to_string());
    }
    if cfg.bucket.is_empty() {
        return Err("未配置 S3 Bucket".to_string());
    }
    if cfg.access_key.is_empty() || cfg.secret_key.is_empty() {
        return Err("未配置 Access Key 或 Secret Key".to_string());
    }
    Ok(())
}
