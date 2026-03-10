//! 其他操作
//!
//! 本模块包含安装以外的应用生命周期操作：
//! - 获取已安装应用列表
//! - 卸载应用
//! - 搜索应用版本
//! - 运行应用

use log::{error, info, warn};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::services::executor;
use crate::services::ll_cli_command;
use crate::services::process::kill_linglong_app;

use super::models::{InstalledApp, LLCliListItem};

/// 获取已安装的玲珑应用列表
///
/// # Arguments
/// * `include_base_service` - 是否包含基础服务（runtime, base）
///
/// # Returns
/// * `Ok(Vec<InstalledApp>)` - 已安装应用列表
/// * `Err(String)` - 获取失败原因
pub async fn get_installed_apps(include_base_service: bool) -> Result<Vec<InstalledApp>, String> {
    let args: Vec<&str> = if include_base_service {
        vec!["list", "--json", "--type=all"]
    } else {
        vec!["list", "--json"]
    };

    let stdout = executor::execute_or_err(&args, "list").await?;
    let trimmed = stdout.trim();

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // 解析 JSON 输出
    let list_items: Vec<LLCliListItem> = serde_json::from_str(trimmed)
        .map_err(|e| format!("Failed to parse ll-cli list output: {}", e))?;

    // 转换为 InstalledApp 结构
    let apps: Vec<InstalledApp> = list_items
        .into_iter()
        .filter(|item| {
            if include_base_service {
                true
            } else {
                // 只保留 kind 为 "app" 的应用
                item.kind.as_ref().map_or(false, |k| k == "app")
            }
        })
        .map(|item| item.into_installed_app())
        .collect();

    Ok(apps)
}

/// 卸载指定的玲珑应用
///
/// # Arguments
/// * `app_id` - 应用 ID
/// * `version` - 版本号
///
/// # Returns
/// * `Ok(String)` - 卸载成功消息
/// * `Err(String)` - 卸载失败原因
pub async fn uninstall_linglong_app(app_id: String, version: String) -> Result<String, String> {
    info!(
        "[Uninstall] Checking and stopping app before uninstall: {}",
        app_id
    );

    // 尝试停止运行中的应用
    if let Err(err) = kill_linglong_app(app_id.clone()).await {
        warn!("[Uninstall] Failed to stop app {}: {}", app_id, err);
        return Err(format!("卸载失败，请先停止应用运行。详情: {}", err));
    }

    info!(
        "[Uninstall] App stopped successfully, proceeding to uninstall: {}",
        app_id
    );

    let app_ref = format!("{}/{}", app_id, version);

    executor::execute_or_err(&["uninstall", &app_ref], "uninstall").await?;

    Ok(format!(
        "Successfully uninstalled {} version {}",
        app_id, version
    ))
}

/// 搜索指定 appId 的所有已安装版本
///
/// # Arguments
/// * `app_id` - 应用 ID
///
/// # Returns
/// * `Ok(Vec<InstalledApp>)` - 匹配的已安装版本列表
/// * `Err(String)` - 搜索失败原因
pub async fn search_app_versions(app_id: String) -> Result<Vec<InstalledApp>, String> {
    info!(
        "[SearchVersions] Searching for installed versions of: {}",
        app_id
    );

    // 使用 executor 获取所有已安装的应用
    let stdout = executor::execute_or_err(
        &["list", "--json", "--type=all"],
        "search_versions",
    ).await.map_err(|e| {
        error!("[SearchVersions] Error: {}", e);
        e
    })?;

    let trimmed = stdout.trim();

    info!("[SearchVersions] Output length: {} bytes", trimmed.len());

    if trimmed.is_empty() {
        warn!("[SearchVersions] Empty output, returning empty vec");
        return Ok(Vec::new());
    }

    // 解析 JSON 输出
    let list_items: Vec<LLCliListItem> = serde_json::from_str(trimmed).map_err(|e| {
        let err_msg = format!("Failed to parse ll-cli list output: {}", e);
        error!("[SearchVersions] Parse error: {}", err_msg);
        err_msg
    })?;

    info!("[SearchVersions] Found {} installed items", list_items.len());

    // 过滤出指定 app_id 的所有版本
    let apps: Vec<InstalledApp> = list_items
        .into_iter()
        .filter(|item| {
            // 匹配 app_id 或 name
            let matches = item.app_id.as_ref().map_or(false, |id| id == &app_id)
                || item.name == app_id;
            if matches {
                info!(
                    "[SearchVersions] Found matching app: {} ({})",
                    item.name,
                    item.app_id.as_ref().unwrap_or(&item.name)
                );
            }
            matches
        })
        .map(|item| item.into_installed_app())
        .collect();

    info!(
        "[SearchVersions] Found {} installed versions for: {}",
        apps.len(),
        app_id
    );

    for app in &apps {
        info!(
            "[SearchVersions] - {} version: {}, channel: {}, module: {}",
            app.app_id, app.version, app.channel, app.module
        );
    }

    Ok(apps)
}

/// 运行指定的玲珑应用
///
/// 根据 ll-cli 文档，run 命令只需要应用名，不需要版本号。
///
/// # Arguments
/// * `app_id` - 应用 ID
///
/// # Returns
/// * `Ok(String)` - 启动成功消息
/// * `Err(String)` - 启动失败原因
pub async fn run_linglong_app(app_id: String) -> Result<String, String> {
    info!("[Run] Starting app: {}", app_id);
    info!("[Run] Command: ll-cli run {}", app_id);

    // 在后台线程中启动命令，不等待退出
    let app_id_bg = app_id.clone();
    std::thread::spawn(move || {
        info!("[Run:bg] Spawning ll-cli run {}", app_id_bg);

        let mut cmd = ll_cli_command();
        let spawn_result = cmd
            .arg("run")
            .arg(&app_id_bg)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        match spawn_result {
            Ok(child) => {
                info!("[Run:bg] Process spawned with PID: {:?}", child.id());
                // 不 wait，让子进程自行运行
            }
            Err(e) => {
                error!(
                    "[Run:bg] Failed to execute 'll-cli run' for {}: {}",
                    app_id_bg, e
                );
            }
        }
    });

    // 立即返回
    Ok(format!("Successfully launched {}", app_id))
}

/// 为已安装应用创建桌面快捷方式（用户级，无需管理员权限）
///
/// 行为说明：
/// - 仅允许已安装应用创建快捷方式
/// - 从 `ll-cli content <app_id>` 输出中解析 `.desktop` 源文件
/// - 复制到 `~/Desktop`
/// - 目标同名文件存在时不覆盖，直接返回提示
pub async fn create_desktop_shortcut(app_id: String) -> Result<String, String> {
    info!("[Shortcut] Creating desktop shortcut for app: {}", app_id);

    let installed_apps = get_installed_apps(false).await?;
    let is_installed = installed_apps.iter().any(|item| item.app_id == app_id);

    if !is_installed {
        return Err(format!("应用未安装，无法创建快捷方式: {}", app_id));
    }

    let stdout = executor::execute_or_err(&["content", &app_id], "content").await?;

    let desktop_source = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.ends_with(".desktop"))
        .map(PathBuf::from)
        .ok_or_else(|| format!("未找到应用导出的 desktop 文件: {}", app_id))?;

    let desktop_file_name = desktop_source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("desktop 文件名无效: {}", desktop_source.display()))?;

    let home_dir = std::env::var("HOME").map_err(|_| "无法获取 HOME 目录".to_string())?;
    let target_dir = PathBuf::from(home_dir).join("Desktop");

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
