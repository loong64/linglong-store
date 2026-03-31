//! 桌面集成
//!
//! 本模块负责创建/管理应用的桌面快捷方式。

use log::info;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::services::executor;
use crate::utils::linux::xdg::resolve_user_desktop_dir;

use super::operations::get_installed_apps;

#[derive(Debug, Default, PartialEq, Eq)]
struct DesktopEntryInspection {
    first_group_is_desktop_entry: bool,
    is_application: bool,
    has_name: bool,
    has_exec: bool,
    dbus_activatable: bool,
}

/// 为指定应用创建桌面快捷方式
///
/// 从 `ll-cli content` 获取 .desktop 文件路径，校验其基础合法性后复制到
/// 符合 XDG 用户目录规范的桌面目录。
pub async fn create_desktop_shortcut(app_id: String) -> Result<String, String> {
    info!("[Shortcut] Creating desktop shortcut for app: {}", app_id);

    let installed_apps = get_installed_apps(false).await?;
    let is_installed = installed_apps.iter().any(|item| item.app_id == app_id);

    if !is_installed {
        return Err(format!("应用未安装，无法创建快捷方式: {}", app_id));
    }

    let stdout = executor::execute_or_err(&["content", &app_id], "content").await?;

    let desktop_source = extract_desktop_source(&stdout, &app_id)?;
    if !desktop_source.exists() {
        return Err(format!(
            "源 desktop 文件不存在: {}",
            desktop_source.display()
        ));
    }
    validate_desktop_entry_file(&desktop_source)?;

    let desktop_file_name = desktop_source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("desktop 文件名无效: {}", desktop_source.display()))?;

    let target_dir = resolve_user_desktop_dir().await?;

    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("创建目标目录失败 {}: {}", target_dir.display(), e))?;

    let target_path = target_dir.join(desktop_file_name);
    if target_path.exists() {
        return Err(format!(
            "快捷方式已存在，不会覆盖: {}",
            target_path.display()
        ));
    }

    fs::copy(&desktop_source, &target_path).map_err(|e| {
        format!(
            "复制 desktop 文件失败: {} -> {} ({})",
            desktop_source.display(),
            target_path.display(),
            e
        )
    })?;

    fs::set_permissions(&target_path, fs::Permissions::from_mode(0o755)).map_err(|e| {
        format!(
            "设置 desktop 文件权限失败: {} ({})",
            target_path.display(),
            e
        )
    })?;

    info!(
        "[Shortcut] Desktop shortcut created for {} at {}",
        app_id,
        target_path.display()
    );

    Ok(format!("已创建桌面快捷方式: {}", target_path.display()))
}

/// 从 `ll-cli content` 输出中提取第一个 `.desktop` 文件路径。
fn extract_desktop_source(stdout: &str, app_id: &str) -> Result<PathBuf, String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.ends_with(".desktop"))
        .map(PathBuf::from)
        .ok_or_else(|| format!("未找到应用导出的 desktop 文件: {}", app_id))
}

/// 校验待复制的 `.desktop` 文件是否满足基础 Desktop Entry 规范。
///
/// 为避免破坏应用导出的原始字段，这里只做只读校验，不改写文件内容。
fn validate_desktop_entry_file(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("读取 desktop 文件失败: {} ({})", path.display(), e))?;
    let inspection = inspect_desktop_entry_content(&content);

    if !inspection.first_group_is_desktop_entry {
        return Err(format!(
            "desktop 文件缺少合法的 [Desktop Entry] 组: {}",
            path.display()
        ));
    }

    if !inspection.is_application {
        return Err(format!(
            "desktop 文件不是 Application 类型: {}",
            path.display()
        ));
    }

    if !inspection.has_name {
        return Err(format!("desktop 文件缺少 Name 字段: {}", path.display()));
    }

    if !inspection.has_exec && !inspection.dbus_activatable {
        return Err(format!("desktop 文件缺少 Exec 字段: {}", path.display()));
    }

    Ok(())
}

/// 提取 Desktop Entry 关键字段，用于基础合规校验。
fn inspect_desktop_entry_content(content: &str) -> DesktopEntryInspection {
    let mut inspection = DesktopEntryInspection::default();
    let mut in_desktop_entry_group = false;
    let mut first_group_seen = false;

    for line in content.lines() {
        let trimmed = line.trim_start_matches('\u{feff}').trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let group_name = &trimmed[1..trimmed.len() - 1];
            if !first_group_seen {
                inspection.first_group_is_desktop_entry = group_name == "Desktop Entry";
                first_group_seen = true;
            }
            in_desktop_entry_group = group_name == "Desktop Entry";
            continue;
        }

        if !in_desktop_entry_group {
            continue;
        }

        let Some((raw_key, raw_value)) = trimmed.split_once('=') else {
            continue;
        };

        let key = raw_key.trim();
        let value = raw_value.trim();

        match key {
            "Type" => inspection.is_application = value == "Application",
            "Name" => inspection.has_name = !value.is_empty(),
            "Exec" => inspection.has_exec = !value.is_empty(),
            "DBusActivatable" => inspection.dbus_activatable = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }

    inspection
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_desktop_source_returns_first_desktop_file() {
        let stdout = "/tmp/demo.txt\n/var/lib/linglong/app/org.demo.App/files/share/applications/org.demo.App.desktop\n";
        let source = extract_desktop_source(stdout, "org.demo.App").unwrap();

        assert_eq!(
            source,
            PathBuf::from(
                "/var/lib/linglong/app/org.demo.App/files/share/applications/org.demo.App.desktop"
            )
        );
    }

    #[test]
    fn test_inspect_desktop_entry_content_detects_valid_application() {
        let content = "[Desktop Entry]\nType=Application\nName=Demo\nExec=demo-app\n";
        let inspection = inspect_desktop_entry_content(content);

        assert!(inspection.first_group_is_desktop_entry);
        assert!(inspection.is_application);
        assert!(inspection.has_name);
        assert!(inspection.has_exec);
        assert!(!inspection.dbus_activatable);
    }

    #[test]
    fn test_inspect_desktop_entry_content_rejects_invalid_group_order() {
        let content =
            "[Extra]\nName=Wrong\n[Desktop Entry]\nType=Application\nName=Demo\nExec=demo-app\n";
        let inspection = inspect_desktop_entry_content(content);

        assert!(!inspection.first_group_is_desktop_entry);
    }

    #[test]
    fn test_inspect_desktop_entry_content_allows_dbus_activation_without_exec() {
        let content = "[Desktop Entry]\nType=Application\nName=Demo\nDBusActivatable=true\n";
        let inspection = inspect_desktop_entry_content(content);

        assert!(inspection.first_group_is_desktop_entry);
        assert!(inspection.is_application);
        assert!(inspection.has_name);
        assert!(!inspection.has_exec);
        assert!(inspection.dbus_activatable);
    }

    #[test]
    fn test_inspect_desktop_entry_content_accepts_utf8_bom() {
        let content = "\u{feff}[Desktop Entry]\nType=Application\nName=Demo\nExec=demo-app\n";
        let inspection = inspect_desktop_entry_content(content);

        assert!(inspection.first_group_is_desktop_entry);
        assert!(inspection.is_application);
        assert!(inspection.has_name);
        assert!(inspection.has_exec);
    }
}
