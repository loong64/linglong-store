use crate::services::ll_cli_command;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LinglongAppInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub channel: String,
    pub source: String,
    pub pid: String,
    pub container_id: String,
}

#[derive(Debug, Deserialize)]
struct AppInfoJson {
    arch: Vec<String>,
    channel: String,
    id: String,
    version: String,
    base: String,
}

pub async fn get_running_linglong_apps() -> Result<Vec<LinglongAppInfo>, String> {
    let ps_output = ll_cli_command()
        .arg("ps")
        .output()
        .map_err(|e| format!("Failed to execute 'll-cli ps': {}", e))?;

    if !ps_output.status.success() {
        return Err(format!(
            "ll-cli ps command failed with status: {}",
            ps_output.status
        ));
    }

    let ps_string = String::from_utf8_lossy(&ps_output.stdout);
    let mut apps = Vec::new();

    // Skip header line
    for line in ps_string.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let app_name = parts[0];
            let container_id = parts[1];
            let pid = parts[2];

            let info_output = ll_cli_command()
                .arg("info")
                .arg(app_name)
                .output()
                .map_err(|e| format!("Failed to execute 'll-cli info {}': {}", app_name, e))?;

            if info_output.status.success() {
                let info_string = String::from_utf8_lossy(&info_output.stdout);
                if let Ok(info_json) = serde_json::from_str::<AppInfoJson>(&info_string) {
                    let source = info_json.base.split(':').next().unwrap_or("").to_string();
                    apps.push(LinglongAppInfo {
                        name: info_json.id,
                        version: info_json.version,
                        arch: info_json.arch.join(", "),
                        channel: info_json.channel,
                        source,
                        pid: pid.to_string(),
                        container_id: container_id.to_string(),
                    });
                }
            }
        }
    }

    Ok(apps)
}

async fn is_app_running(app_id: &str) -> Result<bool, String> {
    let ps_output = ll_cli_command()
        .arg("ps")
        .output()
        .map_err(|e| format!("Failed to execute 'll-cli ps': {}", e))?;

    if !ps_output.status.success() {
        return Err(format!(
            "ll-cli ps command failed with status: {}",
            ps_output.status
        ));
    }

    let ps_string = String::from_utf8_lossy(&ps_output.stdout);
    // Skip header line
    for line in ps_string.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.first() == Some(&app_id) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn kill_linglong_app(app_name: String) -> Result<String, String> {
    // 尝试停止运行中的应用，最多 5 次，间隔 1 秒
    for attempt in 1..=5 {
        let running = is_app_running(&app_name).await?;
        if !running {
            info!("[kill_linglong_app] App not running, proceed: {}", app_name);
            return Ok(format!("Successfully stopped {}", app_name));
        }

        info!(
            "[kill_linglong_app] App is running, attempt {} to kill: {}",
            attempt, app_name
        );
        let output = ll_cli_command()
            .arg("kill")
            .arg("-s")
            .arg("9")
            .arg(&app_name)
            .output()
            .map_err(|e| format!("Failed to execute 'll-cli kill': {}", e))?;
        let mut error_msg = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            warn!(
                "[kill_linglong_app] kill attempt {} failed for {}: {}",
                attempt, app_name, error_msg
            );
        }

        if attempt == 5 {
            // 最后一轮后再检查一次，仍在运行则返回错误
            let still_running = is_app_running(&app_name).await.unwrap_or(true);
            if still_running {
                warn!("[kill_linglong_app] error_msg: {}", error_msg);
                if error_msg.is_empty() {
                    error_msg = "未知错误".to_string();
                }
                return Err(error_msg.to_string());
            }
            break;
        }

        sleep(Duration::from_secs(1)).await;
    }

    Ok(format!("Successfully stopped {}", app_name))
}
