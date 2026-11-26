//! JoBox Support: update check through the JoBox site, the same way the other
//! JoBox programs do it. The file `update_jobox.json` on the site has a section
//! per program; this one reads the "jbsupport" section:
//!
//! ```json
//! "jbsupport": {
//!     "latest_version": "2.2",
//!     "download_url": "https://ячейка-пвз.рф/static/JoBox_Support_Installer.exe",
//!     "release_notes": "...",
//!     "sha256": "optional, checked when present"
//! }
//! ```
//!
//! The installed version is the one the JoBox Support installer (Inno Setup)
//! wrote to the registry, so only the installer script holds the version number.
//! The update is started only by the user (button on the home page): a broken
//! update installed silently everywhere would leave clients without support.

use hbb_common::{bail, log, ResultType};
use serde_derive::Deserialize;
use std::{sync::Mutex, time::Duration};

const UPDATE_JSON_URL: &str = "https://xn----7sbhjnih6b1d2d.xn--p1ai/update_jobox.json";
const SITE_HOST: &str = "xn----7sbhjnih6b1d2d.xn--p1ai";
const APP_ALIAS: &str = "jbsupport";
// AppId of the installer (jbsupport.iss) + "_is1", as written by Inno Setup.
const UNINSTALL_KEY: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{EFD2D89A-3927-4A09-BE5A-A0A4E0D71D35}_is1";
const INSTALLER_ARGS: &str = "/SILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS";

#[derive(Clone, Deserialize)]
struct UpdateInfo {
    latest_version: String,
    download_url: String,
    #[serde(default)]
    release_notes: String,
    #[serde(default)]
    sha256: String,
}

static AVAILABLE: Mutex<Option<UpdateInfo>> = Mutex::new(None);

/// Version of JoBox Support written by the installer, e.g. "2.2".
pub fn installed_version() -> Option<String> {
    use winreg::{enums::*, RegKey};
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(UNINSTALL_KEY, KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    let version: String = key.get_value("DisplayVersion").ok()?;
    let version = version.trim().to_owned();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

fn parse_version(v: &str) -> Option<Vec<u32>> {
    v.trim()
        .split('.')
        .map(|part| part.trim().parse::<u32>().ok())
        .collect()
}

fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(mut remote), Some(mut current)) => {
            let len = remote.len().max(current.len());
            remote.resize(len, 0);
            current.resize(len, 0);
            remote > current
        }
        _ => false,
    }
}

fn check_download_url(url: &str) -> ResultType<()> {
    // `url` converts "ячейка-пвз.рф" to its punycode form.
    let parsed = url::Url::parse(url)?;
    if parsed.scheme() != "https" || parsed.host_str() != Some(SITE_HOST) {
        bail!("Unexpected download url: {}", url);
    }
    Ok(())
}

fn fetch_update_info() -> ResultType<Option<UpdateInfo>> {
    if !crate::platform::is_installed() {
        return Ok(None);
    }
    let Some(current) = installed_version() else {
        return Ok(None);
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let url = format!("{}?t={}", UPDATE_JSON_URL, ts);
    let client = crate::hbbs_http::create_http_client_with_url(&url);
    let resp = client.get(&url).timeout(Duration::from_secs(15)).send()?;
    if !resp.status().is_success() {
        bail!("HTTP status {}", resp.status());
    }
    let data: serde_json::Value = resp.json()?;
    let Some(section) = data.get(APP_ALIAS) else {
        return Ok(None);
    };
    let info: UpdateInfo = serde_json::from_value(section.clone())?;
    if !is_newer(&info.latest_version, &current) {
        return Ok(None);
    }
    check_download_url(&info.download_url)?;
    log::info!(
        "JoBox Support update available: {} -> {}",
        current,
        info.latest_version
    );
    Ok(Some(info))
}

/// Returns `{"version": .., "notes": ..}` when a newer version is published, "" otherwise.
pub fn check() -> String {
    match fetch_update_info() {
        Ok(Some(info)) => {
            let out = serde_json::json!({
                "version": info.latest_version,
                "notes": info.release_notes,
            })
            .to_string();
            *AVAILABLE.lock().unwrap() = Some(info);
            out
        }
        Ok(None) => {
            *AVAILABLE.lock().unwrap() = None;
            String::new()
        }
        Err(e) => {
            log::warn!("JoBox Support update check failed: {}", e);
            String::new()
        }
    }
}

fn sha256_hex(path: &std::path::Path) -> ResultType<String> {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(&std::fs::read(path)?);
    Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
}

fn download_and_run_() -> ResultType<()> {
    let Some(info) = AVAILABLE.lock().unwrap().clone() else {
        bail!("No update available");
    };
    check_download_url(&info.download_url)?;
    let client = crate::hbbs_http::create_http_client_with_url(&info.download_url);
    let mut resp = client
        .get(&info.download_url)
        .timeout(Duration::from_secs(900))
        .send()?;
    if !resp.status().is_success() {
        bail!("HTTP status {}", resp.status());
    }
    let path = std::env::temp_dir().join("JoBoxSupport_update.exe");
    std::fs::remove_file(&path).ok();
    {
        let mut file = std::fs::File::create(&path)?;
        resp.copy_to(&mut file)?;
    }
    let expected = info.sha256.trim();
    if !expected.is_empty() {
        let actual = sha256_hex(&path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            std::fs::remove_file(&path).ok();
            bail!("Checksum mismatch: expected {}, got {}", expected, actual);
        }
    }
    log::info!("Starting JoBox Support installer {:?}", path);
    // "runas": Windows asks the user for permission (UAC), the installer needs admin rights.
    if !crate::platform::windows::run_uac(&path.to_string_lossy(), INSTALLER_ARGS)? {
        bail!("cancelled");
    }
    Ok(())
}

/// Downloads the installer found by `check()` and starts it.
/// Returns "" on success, "error:<text>" otherwise ("error:cancelled" if the user declined UAC).
pub fn download_and_run() -> String {
    match download_and_run_() {
        Ok(()) => String::new(),
        Err(e) => {
            log::error!("JoBox Support update failed: {}", e);
            format!("error:{}", e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("2.2", "2.1"));
        assert!(is_newer("2.10", "2.9"));
        assert!(is_newer("3", "2.9.9"));
        assert!(!is_newer("2.1", "2.1"));
        assert!(!is_newer("2.1.0", "2.1"));
        assert!(!is_newer("2.0", "2.1"));
        assert!(!is_newer("abc", "2.1"));
    }

    #[test]
    fn download_url() {
        assert!(check_download_url("https://ячейка-пвз.рф/static/JoBox_Support_Installer.exe").is_ok());
        assert!(check_download_url(
            "https://xn----7sbhjnih6b1d2d.xn--p1ai/static/JoBox_Support_Installer.exe"
        )
        .is_ok());
        assert!(check_download_url("http://ячейка-пвз.рф/static/x.exe").is_err());
        assert!(check_download_url("https://example.com/x.exe").is_err());
    }
}
