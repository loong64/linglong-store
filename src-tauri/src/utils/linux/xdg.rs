//! XDG 用户目录工具。
//!
//! 当前聚焦桌面目录解析，供桌面快捷方式等 Linux 桌面集成功能复用。

use log::{debug, warn};
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const XDG_DESKTOP_DIR_KEY: &str = "XDG_DESKTOP_DIR";
const XDG_CONFIG_HOME_KEY: &str = "XDG_CONFIG_HOME";
const USER_DIRS_FILE_NAME: &str = "user-dirs.dirs";
const XDG_USER_DIR_TIMEOUT: Duration = Duration::from_secs(3);

/// 根据 XDG 规范解析用户桌面目录。
///
/// 解析顺序：
/// 1. `XDG_DESKTOP_DIR` 环境变量（仅接受绝对路径）
/// 2. `xdg-user-dir DESKTOP` 命令
/// 3. `${XDG_CONFIG_HOME:-$HOME/.config}/user-dirs.dirs`
/// 4. `~/Desktop`
pub async fn resolve_user_desktop_dir() -> Result<PathBuf, String> {
    if let Some(desktop_dir) = resolve_desktop_dir_from_env() {
        return Ok(desktop_dir);
    }

    if let Some(desktop_dir) = resolve_desktop_dir_from_xdg_user_dir().await {
        return Ok(desktop_dir);
    }

    if let Some(desktop_dir) = resolve_desktop_dir_from_user_dirs_file()? {
        return Ok(desktop_dir);
    }

    resolve_home_desktop_fallback()
}

/// 从 `XDG_DESKTOP_DIR` 环境变量解析桌面目录。
///
/// XDG Base Directory 规范要求环境变量值必须是绝对路径，
/// 非绝对路径会被视为无效并继续回退。
fn resolve_desktop_dir_from_env() -> Option<PathBuf> {
    let candidate = env::var_os(XDG_DESKTOP_DIR_KEY).map(PathBuf::from)?;
    normalize_candidate_dir(candidate, XDG_DESKTOP_DIR_KEY)
}

/// 通过 `xdg-user-dir DESKTOP` 解析桌面目录。
///
/// 命令不可用、超时或返回无效路径时会自动回退到下一层策略。
async fn resolve_desktop_dir_from_xdg_user_dir() -> Option<PathBuf> {
    let output = match timeout(
        XDG_USER_DIR_TIMEOUT,
        Command::new("xdg-user-dir").arg("DESKTOP").output(),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            debug!("[XDG] xdg-user-dir 不可用: {}", error);
            return None;
        }
        Err(_) => {
            debug!(
                "[XDG] xdg-user-dir DESKTOP 超时 ({}s)",
                XDG_USER_DIR_TIMEOUT.as_secs()
            );
            return None;
        }
    };

    if !output.status.success() {
        debug!(
            "[XDG] xdg-user-dir DESKTOP 返回非零退出码: {:?}",
            output.status.code()
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return None;
    }

    normalize_candidate_dir(PathBuf::from(stdout), "xdg-user-dir DESKTOP")
}

/// 从 `user-dirs.dirs` 文件解析桌面目录。
///
/// 配置文件位置遵循 XDG Base Directory 规范：
/// `${XDG_CONFIG_HOME:-$HOME/.config}/user-dirs.dirs`。
fn resolve_desktop_dir_from_user_dirs_file() -> Result<Option<PathBuf>, String> {
    let config_home = resolve_xdg_config_home()?;
    let user_dirs_path = config_home.join(USER_DIRS_FILE_NAME);

    let content = match fs::read_to_string(&user_dirs_path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            warn!(
                "[XDG] 读取用户目录配置失败，继续回退到下一层策略: {} ({})",
                user_dirs_path.display(),
                error
            );
            return Ok(None);
        }
    };

    let home_dir = resolve_home_dir()?;
    let parsed = parse_user_dirs_desktop_path(&content, &home_dir);
    Ok(parsed.and_then(|candidate| normalize_candidate_dir(candidate, "user-dirs.dirs")))
}

/// 解析 XDG 配置根目录。
///
/// 若 `XDG_CONFIG_HOME` 缺失或无效，则回退到 `~/.config`。
fn resolve_xdg_config_home() -> Result<PathBuf, String> {
    if let Some(candidate) = env::var_os(XDG_CONFIG_HOME_KEY).map(PathBuf::from) {
        if let Some(config_home) = normalize_candidate_dir(candidate, XDG_CONFIG_HOME_KEY) {
            return Ok(config_home);
        }
    }

    Ok(resolve_home_dir()?.join(".config"))
}

/// 读取 `HOME` 环境变量并转换为绝对路径。
fn resolve_home_dir() -> Result<PathBuf, String> {
    let home_dir = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "无法获取 HOME 目录".to_string())?;

    if !home_dir.is_absolute() {
        return Err(format!(
            "HOME 目录不是绝对路径，无法继续解析 XDG 路径: {}",
            home_dir.display()
        ));
    }

    Ok(home_dir)
}

/// 使用传统 `~/Desktop` 作为最终兜底路径。
fn resolve_home_desktop_fallback() -> Result<PathBuf, String> {
    Ok(resolve_home_dir()?.join("Desktop"))
}

/// 解析 `user-dirs.dirs` 中的 `XDG_DESKTOP_DIR` 配置。
///
/// 配置文件通常使用 shell 风格变量，例如 `$HOME/Desktop` 或 `${HOME}/Desktop`。
fn parse_user_dirs_desktop_path(content: &str, home_dir: &Path) -> Option<PathBuf> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim() != XDG_DESKTOP_DIR_KEY {
                return None;
            }

            let unquoted = strip_surrounding_quotes(value.trim());
            let expanded = expand_home_tokens(unquoted, home_dir);
            Some(PathBuf::from(expanded))
        })
}

/// 去除 shell 风格配置中最外层的引号，保留内部内容。
fn strip_surrounding_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let quoted_with_double = value.starts_with('"') && value.ends_with('"');
        let quoted_with_single = value.starts_with('\'') && value.ends_with('\'');
        if quoted_with_double || quoted_with_single {
            return &value[1..value.len() - 1];
        }
    }

    value
}

/// 将 `user-dirs.dirs` 中的 HOME 占位符展开为实际路径。
fn expand_home_tokens(value: &str, home_dir: &Path) -> String {
    let home = home_dir.to_string_lossy();
    value
        .replace("${HOME}", home.as_ref())
        .replace("$HOME", home.as_ref())
}

/// 过滤无效候选路径，避免把 `/` 或相对路径当作桌面目录。
fn normalize_candidate_dir(candidate: PathBuf, source: &str) -> Option<PathBuf> {
    if candidate.as_os_str().is_empty() {
        return None;
    }

    if !candidate.is_absolute() {
        warn!(
            "[XDG] 忽略非绝对路径的桌面目录来源 {}: {}",
            source,
            candidate.display()
        );
        return None;
    }

    if candidate == Path::new("/") {
        warn!("[XDG] 忽略无效桌面目录来源 {}: /", source);
        return None;
    }

    Some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_dirs_desktop_path_supports_home_token() {
        let content = r#"
            XDG_DOWNLOAD_DIR="$HOME/Downloads"
            XDG_DESKTOP_DIR="$HOME/桌面"
        "#;

        let parsed = parse_user_dirs_desktop_path(content, Path::new("/home/tester")).unwrap();

        assert_eq!(parsed, PathBuf::from("/home/tester/桌面"));
    }

    #[test]
    fn test_parse_user_dirs_desktop_path_supports_braced_home_token() {
        let content = r#"XDG_DESKTOP_DIR="${HOME}/Desktop""#;

        let parsed = parse_user_dirs_desktop_path(content, Path::new("/home/tester")).unwrap();

        assert_eq!(parsed, PathBuf::from("/home/tester/Desktop"));
    }

    #[test]
    fn test_parse_user_dirs_desktop_path_ignores_other_keys() {
        let content = r#"XDG_DOWNLOAD_DIR="$HOME/Downloads""#;

        assert!(parse_user_dirs_desktop_path(content, Path::new("/home/tester")).is_none());
    }

    #[test]
    fn test_normalize_candidate_dir_rejects_relative_and_root_path() {
        assert!(normalize_candidate_dir(PathBuf::from("Desktop"), "test").is_none());
        assert!(normalize_candidate_dir(PathBuf::from("/"), "test").is_none());
    }

    #[test]
    fn test_normalize_candidate_dir_accepts_absolute_path() {
        let normalized = normalize_candidate_dir(PathBuf::from("/home/tester/Desktop"), "test");

        assert_eq!(normalized, Some(PathBuf::from("/home/tester/Desktop")));
    }
}
