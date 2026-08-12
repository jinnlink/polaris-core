use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use polaris_core::db::{open_database_read_only, CURRENT_SCHEMA_VERSION};
use rusqlite::backup::Backup;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "desktop.json";
const CRASH_MARKER_FILE: &str = "running.marker";
const DEFAULT_DATABASE_FILE: &str = "polaris.sqlite";
const PENDING_JOBS_FILE: &str = "background.pending.json";
const LOG_DIR: &str = "logs";
const LOG_FILE: &str = "polaris.log";
const LOG_ROTATED_FILE: &str = "polaris.log.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DesktopConfig {
    pub database_path: Option<PathBuf>,
    #[serde(default)]
    pub database_path_acknowledged: bool,
    #[serde(default)]
    pub startup_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabasePathSource {
    Saved,
    Environment,
    LocalAppData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatabaseResolution {
    pub path: PathBuf,
    pub source: DatabasePathSource,
    pub needs_acknowledgement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StartupDatabaseState {
    Missing,
    Ready {
        user_version: i64,
        upgrade_required: bool,
    },
    Locked {
        message: String,
    },
    Corrupt {
        message: String,
    },
    Newer {
        found: i64,
        supported: i64,
    },
    Unavailable {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrashMarkerReceipt {
    pub previous_run_incomplete: bool,
    pub marker_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatabasePreparation {
    pub state: StartupDatabaseState,
    pub pre_upgrade_backup: Option<PathBuf>,
}

pub fn load_config(app_data_dir: &Path) -> Result<DesktopConfig, String> {
    let path = app_data_dir.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(DesktopConfig::default());
    }
    let bytes = fs::read(&path).map_err(|error| format!("读取桌面配置失败：{error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("桌面配置格式损坏：{error}"))
}

pub fn load_config_recovering(
    app_data_dir: &Path,
) -> Result<(DesktopConfig, Option<PathBuf>), String> {
    match load_config(app_data_dir) {
        Ok(config) => Ok((config, None)),
        Err(_) => {
            let path = app_data_dir.join(CONFIG_FILE);
            let timestamp = unix_timestamp()?;
            let quarantined = app_data_dir.join(format!("desktop.corrupt-{timestamp}.json"));
            fs::rename(&path, &quarantined)
                .map_err(|error| format!("隔离损坏桌面配置失败：{error}"))?;
            Ok((DesktopConfig::default(), Some(quarantined)))
        }
    }
}

pub fn save_config(app_data_dir: &Path, config: &DesktopConfig) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|error| format!("创建配置目录失败：{error}"))?;
    let path = app_data_dir.join(CONFIG_FILE);
    let temporary = app_data_dir.join(format!("{CONFIG_FILE}.tmp"));
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes).map_err(|error| format!("写入临时配置失败：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("替换旧配置失败：{error}"))?;
    }
    fs::rename(&temporary, &path).map_err(|error| format!("提交桌面配置失败：{error}"))
}

pub fn resolve_database_path(
    config: &DesktopConfig,
    environment_path: Option<&Path>,
    local_app_data: &Path,
) -> Result<DatabaseResolution, String> {
    let (candidate, source) = if let Some(saved) = config.database_path.as_deref() {
        (saved.to_path_buf(), DatabasePathSource::Saved)
    } else if let Some(environment) = environment_path.filter(|path| !path.as_os_str().is_empty()) {
        (environment.to_path_buf(), DatabasePathSource::Environment)
    } else {
        (
            local_app_data.join(DEFAULT_DATABASE_FILE),
            DatabasePathSource::LocalAppData,
        )
    };
    let path = absolute_without_creating(&candidate)?;
    Ok(DatabaseResolution {
        path,
        source,
        needs_acknowledgement: !config.database_path_acknowledged,
    })
}

pub fn inspect_database(path: &Path) -> StartupDatabaseState {
    if !path.exists() {
        return StartupDatabaseState::Missing;
    }
    let connection = match open_database_read_only(path) {
        Ok(connection) => connection,
        Err(error) => return classify_database_error(error.to_string()),
    };
    let user_version = match connection.query_row("PRAGMA user_version", [], |row| row.get(0)) {
        Ok(version) => version,
        Err(error) => return classify_database_error(error.to_string()),
    };
    if user_version > CURRENT_SCHEMA_VERSION {
        return StartupDatabaseState::Newer {
            found: user_version,
            supported: CURRENT_SCHEMA_VERSION,
        };
    }
    let quick_check =
        connection.query_row("PRAGMA quick_check(1)", [], |row| row.get::<_, String>(0));
    match quick_check {
        Ok(message) if message == "ok" => StartupDatabaseState::Ready {
            user_version,
            upgrade_required: user_version < CURRENT_SCHEMA_VERSION,
        },
        Ok(message) => StartupDatabaseState::Corrupt { message },
        Err(error) => classify_database_error(error.to_string()),
    }
}

pub fn prepare_database_for_open(
    path: &Path,
    backup_dir: &Path,
) -> Result<DatabasePreparation, String> {
    let state = inspect_database(path);
    let pre_upgrade_backup = match state {
        StartupDatabaseState::Ready {
            upgrade_required: true,
            ..
        } => Some(backup_database_before_upgrade(path, backup_dir)?),
        _ => None,
    };
    Ok(DatabasePreparation {
        state,
        pre_upgrade_backup,
    })
}

pub fn begin_run(app_data_dir: &Path) -> Result<CrashMarkerReceipt, String> {
    fs::create_dir_all(app_data_dir).map_err(|error| format!("创建运行目录失败：{error}"))?;
    let marker_path = app_data_dir.join(CRASH_MARKER_FILE);
    let previous_run_incomplete = marker_path.exists();
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    fs::write(&marker_path, format!("{started_at}\n"))
        .map_err(|error| format!("写入崩溃标记失败：{error}"))?;
    Ok(CrashMarkerReceipt {
        previous_run_incomplete,
        marker_path,
    })
}

pub fn finish_run(marker_path: &Path) -> Result<(), String> {
    match fs::remove_file(marker_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("清除崩溃标记失败：{error}")),
    }
}

pub fn load_pending_jobs(app_data_dir: &Path) -> Result<Vec<String>, String> {
    let path = app_data_dir.join(PENDING_JOBS_FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path).map_err(|error| format!("读取后台任务恢复记录失败：{error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("后台任务恢复记录损坏：{error}"))
}

pub fn save_pending_jobs(app_data_dir: &Path, jobs: &[String]) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|error| format!("创建任务恢复目录失败：{error}"))?;
    let path = app_data_dir.join(PENDING_JOBS_FILE);
    if jobs.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("清理后台任务恢复记录失败：{error}")),
        };
    }
    let temporary = app_data_dir.join(format!("{PENDING_JOBS_FILE}.tmp"));
    let bytes = serde_json::to_vec_pretty(jobs).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes).map_err(|error| format!("写入任务恢复记录失败：{error}"))?;
    if path.exists() {
        fs::remove_file(&path).map_err(|error| format!("替换任务恢复记录失败：{error}"))?;
    }
    fs::rename(temporary, path).map_err(|error| format!("提交任务恢复记录失败：{error}"))
}

pub fn append_redacted_log(
    app_data_dir: &Path,
    message: &str,
    secrets: &[&str],
    max_bytes: u64,
) -> Result<(), String> {
    let log_dir = app_data_dir.join(LOG_DIR);
    fs::create_dir_all(&log_dir).map_err(|error| format!("创建日志目录失败：{error}"))?;
    let path = log_dir.join(LOG_FILE);
    if path
        .metadata()
        .is_ok_and(|metadata| metadata.len() >= max_bytes)
    {
        let rotated = log_dir.join(LOG_ROTATED_FILE);
        if rotated.exists() {
            fs::remove_file(&rotated).map_err(|error| format!("清理旧轮转日志失败：{error}"))?;
        }
        fs::rename(&path, rotated).map_err(|error| format!("轮转日志失败：{error}"))?;
    }
    let timestamp = unix_timestamp()?;
    let line = format!("{timestamp} {}\n", redact_sensitive(message, secrets));
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("打开日志失败：{error}"))?;
    file.write_all(line.as_bytes())
        .map_err(|error| format!("写入日志失败：{error}"))
}

pub fn export_diagnostic_bundle(
    app_data_dir: &Path,
    output_path: &Path,
    database_path: &Path,
    startup_state: &StartupDatabaseState,
    pending_jobs: &[String],
    secrets: &[&str],
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建诊断包目录失败：{error}"))?;
    }
    let config = load_config(app_data_dir).unwrap_or_default();
    let log = [LOG_ROTATED_FILE, LOG_FILE]
        .iter()
        .filter_map(|name| fs::read_to_string(app_data_dir.join(LOG_DIR).join(name)).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let bundle = serde_json::json!({
        "generated_at": unix_timestamp()?,
        "platform": std::env::consts::OS,
        "database_path": database_path.display().to_string(),
        "database_state": startup_state,
        "pending_background_jobs": pending_jobs,
        "desktop_config": config,
        "redacted_log": redact_sensitive(&log, secrets),
    });
    let bytes = serde_json::to_vec_pretty(&bundle).map_err(|error| error.to_string())?;
    fs::write(output_path, bytes).map_err(|error| format!("写入诊断包失败：{error}"))
}

pub fn redact_sensitive(input: &str, secrets: &[&str]) -> String {
    let mut output = redact_assignment(input, "POLARIS_LLM_FAST_API_KEY");
    output = redact_assignment(&output, "POLARIS_LLM_STRONG_API_KEY");
    output = redact_assignment(&output, "POLARIS_EMBED_API_KEY");
    output = redact_bearer(&output);
    for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
        output = output.replace(secret, "[REDACTED]");
    }
    output
}

fn redact_assignment(input: &str, key: &str) -> String {
    input
        .split_whitespace()
        .map(|part| {
            if part
                .get(..key.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key))
                && part.as_bytes().get(key.len()) == Some(&b'=')
            {
                format!("{key}=[REDACTED]")
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_bearer(input: &str) -> String {
    let words = input.split_whitespace().collect::<Vec<_>>();
    let mut redacted = Vec::with_capacity(words.len());
    let mut index = 0;
    while index < words.len() {
        if words[index].eq_ignore_ascii_case("bearer") && index + 1 < words.len() {
            redacted.push("Bearer".to_owned());
            redacted.push("[REDACTED]".to_owned());
            index += 2;
        } else {
            redacted.push(words[index].to_owned());
            index += 1;
        }
    }
    redacted.join(" ")
}

fn classify_database_error(message: String) -> StartupDatabaseState {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("locked") || normalized.contains("busy") {
        StartupDatabaseState::Locked { message }
    } else if normalized.contains("malformed") || normalized.contains("not a database") {
        StartupDatabaseState::Corrupt { message }
    } else {
        StartupDatabaseState::Unavailable { message }
    }
}

fn absolute_without_creating(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::path::absolute(path).map_err(|error| format!("数据库路径无效：{error}"))
    }
}

fn unix_timestamp() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| error.to_string())
}

fn backup_database_before_upgrade(path: &Path, backup_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(backup_dir).map_err(|error| format!("创建升级备份目录失败：{error}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let destination = backup_dir.join(format!("pre-upgrade-{timestamp}.sqlite"));
    if destination.exists() {
        return Err(format!("升级备份目标已存在：{}", destination.display()));
    }
    backup_database_to(path, &destination)?;
    Ok(destination)
}

pub fn backup_database_to(path: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!("备份目标已存在：{}", destination.display()));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建备份目录失败：{error}"))?;
    }
    let source = open_database_read_only(path).map_err(|error| error.to_string())?;
    let mut target = Connection::open(destination).map_err(|error| error.to_string())?;
    {
        let backup = Backup::new(&source, &mut target).map_err(|error| error.to_string())?;
        backup
            .run_to_completion(128, std::time::Duration::from_millis(10), None)
            .map_err(|error| error.to_string())?;
    }
    target
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())
        .and_then(|message| {
            if message == "ok" {
                Ok(())
            } else {
                Err(format!("升级备份完整性检查失败：{message}"))
            }
        })?;
    Ok(())
}

#[cfg(windows)]
pub mod windows {
    use std::io;
    use std::ptr::null_mut;

    use windows_registry::CURRENT_USER;
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const STARTUP_VALUE: &str = "Polaris";

    pub fn write_credential(target: &str, username: &str, secret: &str) -> Result<(), String> {
        let mut target = wide(target);
        let mut username = wide(username);
        let mut blob = secret.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            CredentialBlobSize: u32::try_from(blob.len()).map_err(|error| error.to_string())?,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: username.as_mut_ptr(),
            ..Default::default()
        };
        let success = unsafe { CredWriteW(&credential, 0) };
        blob.fill(0);
        if success == 0 {
            Err(last_error("写入 Windows Credential Manager 失败"))
        } else {
            Ok(())
        }
    }

    pub fn read_credential(target: &str) -> Result<Option<String>, String> {
        let target = wide(target);
        let mut pointer: *mut CREDENTIALW = null_mut();
        let success = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) };
        if success == 0 {
            let code = unsafe { GetLastError() };
            return if code == ERROR_NOT_FOUND {
                Ok(None)
            } else {
                Err(io::Error::from_raw_os_error(code as i32).to_string())
            };
        }
        let credential = unsafe { &*pointer };
        let bytes = unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        };
        let result = String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|error| format!("Credential Manager 内容不是 UTF-8：{error}"));
        unsafe { CredFree(pointer.cast()) };
        result
    }

    pub fn delete_credential(target: &str) -> Result<bool, String> {
        let target = wide(target);
        let success = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if success != 0 {
            return Ok(true);
        }
        let code = unsafe { GetLastError() };
        if code == ERROR_NOT_FOUND {
            Ok(false)
        } else {
            Err(io::Error::from_raw_os_error(code as i32).to_string())
        }
    }

    pub fn startup_enabled() -> Result<bool, String> {
        startup_enabled_for_value(STARTUP_VALUE)
    }

    pub(crate) fn startup_enabled_for_value(value_name: &str) -> Result<bool, String> {
        let key = match CURRENT_USER.open(RUN_KEY) {
            Ok(key) => key,
            Err(_) => return Ok(false),
        };
        Ok(key.get_string(value_name).is_ok())
    }

    pub fn set_startup_enabled(enabled: bool, executable: &str) -> Result<(), String> {
        set_startup_enabled_for_value(STARTUP_VALUE, enabled, executable)
    }

    pub(crate) fn set_startup_enabled_for_value(
        value_name: &str,
        enabled: bool,
        executable: &str,
    ) -> Result<(), String> {
        if enabled {
            let key = CURRENT_USER
                .create(RUN_KEY)
                .map_err(|error| error.to_string())?;
            key.set_string(value_name, format!("\"{executable}\" --background"))
                .map_err(|error| error.to_string())
        } else {
            let key = match CURRENT_USER.create(RUN_KEY) {
                Ok(key) => key,
                Err(_) => return Ok(()),
            };
            match key.remove_value(value_name) {
                Ok(()) => Ok(()),
                Err(_) => Ok(()),
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    fn last_error(context: &str) -> String {
        let code = unsafe { GetLastError() };
        format!("{context}：{}", io::Error::from_raw_os_error(code as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polaris_core::db::open_database;
    use tempfile::tempdir;

    #[test]
    fn resolves_saved_then_environment_then_local_app_data() {
        let dir = tempdir().unwrap();
        let saved = dir.path().join("saved.sqlite");
        let environment = dir.path().join("environment.sqlite");
        let mut config = DesktopConfig {
            database_path: Some(saved.clone()),
            ..DesktopConfig::default()
        };
        assert_eq!(
            resolve_database_path(&config, Some(&environment), dir.path())
                .unwrap()
                .source,
            DatabasePathSource::Saved
        );
        config.database_path = None;
        assert_eq!(
            resolve_database_path(&config, Some(&environment), dir.path())
                .unwrap()
                .path,
            environment
        );
        let fallback = resolve_database_path(&config, None, dir.path()).unwrap();
        assert_eq!(fallback.path, dir.path().join(DEFAULT_DATABASE_FILE));
        assert!(fallback.needs_acknowledgement);
    }

    #[test]
    fn config_round_trip_never_contains_secrets() {
        let dir = tempdir().unwrap();
        let config = DesktopConfig {
            database_path: Some(dir.path().join("learning.sqlite")),
            database_path_acknowledged: true,
            startup_enabled: false,
        };
        save_config(dir.path(), &config).unwrap();
        assert_eq!(load_config(dir.path()).unwrap(), config);
        let raw = fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        assert!(!raw.to_ascii_lowercase().contains("api_key"));
    }

    #[test]
    fn corrupt_config_is_quarantined_without_blocking_startup() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(CONFIG_FILE), "{broken").unwrap();
        let (config, quarantined) = load_config_recovering(dir.path()).unwrap();
        assert_eq!(config, DesktopConfig::default());
        assert!(quarantined.unwrap().is_file());
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }

    #[test]
    fn pending_jobs_survive_restart_and_clear_after_completion() {
        let dir = tempdir().unwrap();
        let jobs = vec!["grade_queue".to_owned(), "backup".to_owned()];
        save_pending_jobs(dir.path(), &jobs).unwrap();
        assert_eq!(load_pending_jobs(dir.path()).unwrap(), jobs);
        save_pending_jobs(dir.path(), &[]).unwrap();
        assert!(load_pending_jobs(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn logs_rotate_and_diagnostic_export_remains_redacted() {
        let dir = tempdir().unwrap();
        append_redacted_log(
            dir.path(),
            "POLARIS_LLM_FAST_API_KEY=secret first",
            &["secret"],
            1,
        )
        .unwrap();
        append_redacted_log(
            dir.path(),
            "Authorization: Bearer token second",
            &["token"],
            1,
        )
        .unwrap();
        assert!(dir.path().join(LOG_DIR).join(LOG_ROTATED_FILE).is_file());
        let output = dir.path().join("diagnostic.json");
        export_diagnostic_bundle(
            dir.path(),
            &output,
            &dir.path().join("learning.sqlite"),
            &StartupDatabaseState::Missing,
            &["backup".to_owned()],
            &["secret", "token"],
        )
        .unwrap();
        let exported = fs::read_to_string(output).unwrap();
        assert!(!exported.contains("secret"));
        assert!(!exported.contains("token"));
        assert!(exported.contains("[REDACTED]"));
        assert!(exported.contains("backup"));
    }

    #[test]
    fn inspection_distinguishes_missing_ready_upgrade_and_newer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("learning.sqlite");
        assert_eq!(inspect_database(&path), StartupDatabaseState::Missing);
        let connection = open_database(&path).unwrap();
        drop(connection);
        assert_eq!(
            inspect_database(&path),
            StartupDatabaseState::Ready {
                user_version: CURRENT_SCHEMA_VERSION,
                upgrade_required: false,
            }
        );
        let connection = open_database_read_only(&path).unwrap();
        drop(connection);
        let raw = rusqlite_for_test(&path);
        raw.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
            .unwrap();
        drop(raw);
        assert_eq!(
            inspect_database(&path),
            StartupDatabaseState::Newer {
                found: CURRENT_SCHEMA_VERSION + 1,
                supported: CURRENT_SCHEMA_VERSION,
            }
        );
    }

    #[test]
    fn inspection_distinguishes_corrupt_and_locked_databases() {
        let dir = tempdir().unwrap();
        let corrupt = dir.path().join("corrupt.sqlite");
        fs::write(&corrupt, b"this is not sqlite").unwrap();
        assert!(matches!(
            inspect_database(&corrupt),
            StartupDatabaseState::Corrupt { .. }
        ));

        let locked = dir.path().join("locked.sqlite");
        let connection = open_database(&locked).unwrap();
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
        connection
            .pragma_update(None, "locking_mode", "EXCLUSIVE")
            .unwrap();
        connection.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let state = inspect_database(&locked);
        assert!(
            matches!(state, StartupDatabaseState::Locked { .. }),
            "unexpected state: {state:?}"
        );
        connection.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn existing_database_is_backed_up_before_upgrade_and_keeps_data() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("existing.sqlite");
        let connection = open_database(&path).unwrap();
        connection
            .execute(
                "INSERT INTO meta(key, value) VALUES('user.setting', 'keep')",
                [],
            )
            .unwrap();
        connection.pragma_update(None, "user_version", 8).unwrap();
        drop(connection);

        let preparation = prepare_database_for_open(&path, &dir.path().join("backups")).unwrap();
        assert!(matches!(
            preparation.state,
            StartupDatabaseState::Ready {
                user_version: 8,
                upgrade_required: true
            }
        ));
        let backup_path = preparation.pre_upgrade_backup.unwrap();
        let backup = Connection::open(backup_path).unwrap();
        assert_eq!(
            backup
                .query_row(
                    "SELECT value FROM meta WHERE key='user.setting'",
                    [],
                    |row| { row.get::<_, String>(0) }
                )
                .unwrap(),
            "keep"
        );
        drop(backup);

        let migrated = open_database(&path).unwrap();
        assert_eq!(
            migrated
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(
            migrated
                .query_row(
                    "SELECT value FROM meta WHERE key='user.setting'",
                    [],
                    |row| { row.get::<_, String>(0) }
                )
                .unwrap(),
            "keep"
        );
    }

    #[test]
    fn crash_marker_reports_unclean_previous_run_and_clears_idempotently() {
        let dir = tempdir().unwrap();
        let first = begin_run(dir.path()).unwrap();
        assert!(!first.previous_run_incomplete);
        let second = begin_run(dir.path()).unwrap();
        assert!(second.previous_run_incomplete);
        finish_run(&second.marker_path).unwrap();
        finish_run(&second.marker_path).unwrap();
    }

    #[test]
    fn redaction_removes_environment_bearer_and_known_secret_values() {
        let text =
            "POLARIS_LLM_FAST_API_KEY=fast-secret Authorization: Bearer token-123 body-secret";
        let redacted = redact_sensitive(text, &["body-secret"]);
        assert!(!redacted.contains("fast-secret"));
        assert!(!redacted.contains("token-123"));
        assert!(!redacted.contains("body-secret"));
        assert!(redacted.matches("[REDACTED]").count() >= 3);
    }

    #[cfg(windows)]
    #[test]
    fn windows_credential_manager_round_trips_and_deletes_secret() {
        let target = format!(
            "Polaris/Test/{}/{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        windows::write_credential(&target, "test", "secret-value").unwrap();
        assert_eq!(
            windows::read_credential(&target).unwrap().as_deref(),
            Some("secret-value")
        );
        assert!(windows::delete_credential(&target).unwrap());
        assert_eq!(windows::read_credential(&target).unwrap(), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_startup_switch_is_opt_in_and_reversible() {
        let value_name = format!(
            "PolarisTest-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        assert!(!windows::startup_enabled_for_value(&value_name).unwrap());
        windows::set_startup_enabled_for_value(
            &value_name,
            true,
            r"C:\Program Files\Polaris\polaris.exe",
        )
        .unwrap();
        assert!(windows::startup_enabled_for_value(&value_name).unwrap());
        windows::set_startup_enabled_for_value(&value_name, false, "").unwrap();
        assert!(!windows::startup_enabled_for_value(&value_name).unwrap());
    }

    fn rusqlite_for_test(path: &Path) -> rusqlite::Connection {
        rusqlite::Connection::open(path).unwrap()
    }
}
