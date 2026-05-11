/// 容器内无感自动更新模块
///
/// 仅在 LINYAPS_CONTAINER=yes 时运行，全程后台静默执行，任何失败只记录 warn 日志，
/// 不影响主进程正常启动与使用。
///
/// 更新流程：
/// 1. 检测 LINYAPS_CONTAINER 环境变量
/// 2. 从 Gitee API 获取最新 Release tag
/// 3. semver 比较，有新版本才继续
/// 4. 下载裸二进制到临时文件，chmod +x，原子 rename 替换目标位置
/// 5. 下次用户启动脚本时（setup-linyaps-dbus.sh），直接使用新版本

use log::{info, warn};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// 更新就绪事件名，前端通过此事件感知可重启
const SELF_UPDATE_READY_EVENT: &str = "self-update-ready";

/// Gitee Latest Release API 响应（只取 tag_name）
#[derive(Deserialize)]
struct GiteeRelease {
    tag_name: String,
}

/// Gitee 最新 Release API 地址
const GITEE_API_LATEST: &str =
    "https://gitee.com/api/v5/repos/Shirosu/linglong-store/releases/latest";

/// Gitee Release 下载地址前缀
const GITEE_DOWNLOAD_BASE: &str =
    "https://gitee.com/Shirosu/linglong-store/releases/download";

/// 玲珑包名，与启动脚本 BIN_DIR 保持一致
const PACKAGE_NAME: &str = "com.dongpl.linglong-store.v2";

/// 将 uname -m 风格架构名映射为下载文件的后缀
fn arch_suffix() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("amd64"),
        "aarch64" => Some("arm64"),
        "loongarch64" => Some("loong64"),
        _ => None,
    }
}

/// 目标二进制路径：$HOME/.local/share/<PACKAGE_NAME>/linglong-store
/// 与启动脚本 `cp linglong-store "$BIN_DIR"` 保持一致
fn target_bin_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".local/share")
            .join(PACKAGE_NAME)
            .join("linglong-store"),
    )
}

/// semver 比较：latest 是否比 current 新
/// 去除 'v' 前缀后按 major.minor.patch 三段数字比较
fn is_newer(current: &str, latest: &str) -> bool {
    let current = current.trim_start_matches('v');
    let latest = latest.trim_start_matches('v');
    let parse = |s: &str| -> Option<(u64, u64, u64)> {
        let mut it = s.split('.');
        let major: u64 = it.next()?.parse().ok()?;
        let minor: u64 = it.next()?.parse().ok()?;
        // patch 可能带有 pre-release 后缀，只取数字部分
        let patch: u64 = it
            .next()?
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()?;
        Some((major, minor, patch))
    };

    match (parse(current), parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// 对外暴露的入口：在后台 tokio 任务中调用，全程不 panic
///
/// `app_handle`：Tauri AppHandle，更新就绪后用来向前端发送事件
pub async fn run_silent_self_update(app_handle: AppHandle) {
    // 只在容器环境内执行
    let in_container = std::env::var("LINYAPS_CONTAINER")
        .map(|v| v == "yes" || v == "1" || v == "true")
        .unwrap_or(false);

    if !in_container {
        info!("[self_update] 非容器环境，跳过自动更新检查");
        return;
    }

    info!("[self_update] 检测到容器环境，开始后台检查更新");

    match do_update().await {
        Ok(Some(new_version)) => {
            // 通知前端有新版本可用，前端决定是否弹窗询问重启
            if let Err(e) = app_handle.emit(SELF_UPDATE_READY_EVENT, &new_version) {
                warn!("[self_update] 发送更新就绪事件失败: {}", e);
            }
        }
        Ok(None) => {
            // 已是最新版本，无需处理
        }
        Err(e) => {
            warn!("[self_update] 后台更新失败（不影响当前使用）: {}", e);
        }
    }
}

/// 实际的更新逻辑
///
/// 返回：
/// - `Ok(Some(version))` 表示下载并替换成功，携带新版本号
/// - `Ok(None)` 表示已是最新，未做任何操作
/// - `Err(...)` 表示过程中出现错误（安全忽略）
async fn do_update() -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let current_version = env!("CARGO_PKG_VERSION");
    info!("[self_update] 当前版本: {}", current_version);

    let target_path = target_bin_path().ok_or("无法确定目标二进制路径（HOME 未设置？）")?;
    info!("[self_update] 目标路径: {:?}", target_path);

    let arch = arch_suffix().ok_or("不支持的系统架构，仅支持 amd64 / arm64 / loong64")?;

    // 构建带 15s 超时的 HTTP 客户端
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("linglong-store-self-updater")
        .build()?;

    // 查询 Gitee Latest Release
    let release: GiteeRelease = client
        .get(GITEE_API_LATEST)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    info!("[self_update] 远端最新版本: {}", release.tag_name);

    // 版本比较
    if !is_newer(current_version, &release.tag_name) {
        info!("[self_update] 已是最新版本，无需更新");
        return Ok(None);
    }

    info!(
        "[self_update] 发现新版本 {}，准备后台下载",
        release.tag_name
    );

    // 构造下载 URL：https://gitee.com/.../releases/download/<tag>/linglong-store-<arch>
    let file_name = format!("linglong-store-{}", arch);
    let download_url = format!("{}/{}/{}", GITEE_DOWNLOAD_BASE, release.tag_name, file_name);
    info!("[self_update] 下载地址: {}", download_url);

    // 确保目标目录存在
    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // 下载到临时文件（同目录，rename 是原子操作）
    let temp_path = target_path.with_extension("download");

    let bytes = client
        .get(&download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    tokio::fs::write(&temp_path, &bytes).await?;

    // chmod +x
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&temp_path).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&temp_path, perms).await?;
    }

    // 原子替换：Linux 下 rename 即使目标正在运行也安全
    tokio::fs::rename(&temp_path, &target_path).await?;

    info!(
        "[self_update] 更新完成！版本 {} 已就绪，等待用户重启",
        release.tag_name
    );

    Ok(Some(release.tag_name))
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("2.1.1", "2.1.2"));
        assert!(is_newer("2.1.2", "2.2.0"));
        assert!(is_newer("v2.1.1", "v2.2.0"));
        assert!(!is_newer("2.1.2", "2.1.2")); // 同版本，不更新
        assert!(!is_newer("2.2.0", "2.1.9")); // 当前更新，不回退
    }
}
