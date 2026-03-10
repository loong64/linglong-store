use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::services::executor;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LinglongRepo {
    pub name: String,
    pub url: String,
    pub alias: Option<String>,
    pub priority: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LinglongEnvCheckResult {
    pub ok: bool,
    pub reason: Option<String>,
    pub arch: Option<String>,
    pub os_version: Option<String>,
    pub glibc_version: Option<String>,
    pub kernel_info: Option<String>,
    pub detail_msg: Option<String>,
    pub ll_version: Option<String>,
    pub ll_bin_version: Option<String>,
    pub repo_name: Option<String>,
    pub repos: Vec<LinglongRepo>,
    pub is_container: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallLinglongResult {
    pub stdout: String,
    pub stderr: String,
}

fn parse_repo_output(output: &str) -> LinglongEnvCheckResult {
    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return LinglongEnvCheckResult {
            ok: false,
            reason: Some("未检测到仓库信息".to_string()),
            ..Default::default()
        };
    }

    let default_repo = lines
        .get(0)
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string());
    let repo_lines = if lines.len() > 2 { &lines[2..] } else { &[] };
    let mut repos = Vec::new();
    for line in repo_lines {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let name = parts.get(0).unwrap_or(&"").to_string();
        let url = parts.get(1).unwrap_or(&"").to_string();
        let alias = parts.get(2).map(|s| s.to_string());
        let priority = parts.get(3).map(|s| s.to_string());
        repos.push(LinglongRepo {
            name,
            url,
            alias,
            priority,
        });
    }

    LinglongEnvCheckResult {
        ok: repos.len() > 0,
        reason: None,
        repo_name: default_repo,
        repos,
        ..Default::default()
    }
}

fn parse_ll_version(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    if let Ok(json) = serde_json::from_str::<HashMap<String, String>>(raw) {
        if let Some(v) = json.get("version") {
            return Some(v.trim().to_string());
        }
    }
    let cleaned = raw.trim();
    let mut version = None;
    // 提取连续的版本号片段（仅数字和 .）
    if let Some(seg) = cleaned
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|seg| seg.contains('.'))
    {
        if !seg.is_empty() {
            version = Some(seg.to_string());
        }
    }

    if version.is_none() {
        None
    } else {
        version
    }
}

fn get_ll_cli_version_inner() -> Result<String, String> {
    let output = executor::execute(&["--json", "--version"], "version")?;

    if output.success {
        if let Some(v) = parse_ll_version(&output.stdout) {
            return Ok(v);
        }
    }

    Err("无法解析玲珑版本，请确认 ll-cli 可用".to_string())
}

pub async fn get_ll_cli_version() -> Result<String, String> {
    get_ll_cli_version_inner()
}

fn parse_glibc_version(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    // 取首行，避免多余信息干扰
    let first_line = raw.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return None;
    }

    // 尝试从首行中找到形如 2.35 的片段
    if let Some(token) = first_line
        .split_whitespace()
        .rev()
        .find(|part| part.chars().all(|c| c.is_ascii_digit() || c == '.'))
    {
        if token.contains('.') {
            return Some(token.to_string());
        }
    }

    Some(first_line.to_string())
}

fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let to_parts = |v: &str| -> Vec<i32> {
        v.split(|c| c == '.' || c == '-' || c == '_')
            .filter_map(|p| p.parse::<i32>().ok())
            .collect()
    };
    let a = to_parts(v1);
    let b = to_parts(v2);
    let len = a.len().max(b.len());
    for i in 0..len {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        match av.cmp(&bv) {
            std::cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }
    std::cmp::Ordering::Equal
}

pub async fn check_linglong_env(min_version: &str) -> Result<LinglongEnvCheckResult, String> {
    let mut result = LinglongEnvCheckResult::default();

    // 获取架构
    if let Ok(output) = Command::new("uname").arg("-m").output() {
        if output.status.success() {
            result.arch = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    // 获取 OS 信息
    let os_release = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("PRETTY_NAME"))
                .and_then(|line| line.split('=').nth(1))
                .map(|v| v.trim_matches('"').to_string())
        });
    if let Some(name) = os_release {
        result.os_version = Some(name);
    } else if let Ok(output) = Command::new("uname").arg("-a").output() {
        if output.status.success() {
            result.os_version = Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
    }

    // 获取 glibc 版本（ldd --version）
    let glibc_version = Command::new("ldd")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            if !output.status.success() {
                return None;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            parse_glibc_version(&stdout).or_else(|| parse_glibc_version(&stderr))
        })
        .unwrap_or_else(|| "N/A".to_string());
    result.glibc_version = Some(glibc_version.clone());

    // 获取内核信息（uname -a）
    let kernel_info = Command::new("uname")
        .arg("-a")
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "N/A".to_string());
    result.kernel_info = Some(kernel_info.clone());
    if result.os_version.is_none() && kernel_info != "N/A" {
        result.os_version = Some(kernel_info);
    }

    // dpkg 信息（仅作为日志展示）
    if let Ok(output) = Command::new("bash")
        .arg("-c")
        .arg("dpkg -l | grep linglong")
        .output()
    {
        if output.status.success() {
            result.detail_msg = Some(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    // 检查 ll-cli 是否存在
    let exists = executor::execute(&["--help"], "env_check_help");
    if exists.is_err() || !exists.as_ref().unwrap().success {
        result.ok = false;
        result.reason = Some("检测到系统未安装玲珑环境，请先安装".to_string());
        return Ok(result);
    }

    // 仓库信息
    let repo_output = executor::execute(&["--json", "repo", "show"], "env_repo");
    let mut repo_info = LinglongEnvCheckResult::default();
    if let Ok(output) = repo_output {
        if output.success {
            if let Ok(json) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&output.stdout) {
                let repo_name = json
                    .get("defaultRepo")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let repos = json
                    .get("repos")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|item| {
                                Some(LinglongRepo {
                                    name: item.get("name")?.as_str().unwrap_or("").to_string(),
                                    url: item.get("url")?.as_str().unwrap_or("").to_string(),
                                    alias: item.get("alias").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    priority: item.get("priority").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                repo_info.repo_name = repo_name;
                repo_info.repos = repos;
            } else {
                repo_info = parse_repo_output(&output.stdout);
            }
        } else {
            // 尝试旧命令
            if let Ok(fallback) = executor::execute(&["repo", "show"], "env_repo_fallback") {
                if fallback.success {
                    repo_info = parse_repo_output(&fallback.stdout);
                }
            }
        }
    }
    if repo_info.repos.is_empty() {
        result.ok = false;
        result.reason = Some("未检测到玲珑仓库配置，请检查环境".to_string());
        return Ok(result);
    }
    result.repo_name = repo_info.repo_name.clone();
    result.repos = repo_info.repos.clone();

    // 获取 ll-cli 版本
    let version = get_ll_cli_version_inner().ok();
    result.ll_version = version.clone();

    // 获取 linglong-bin 版本（APT 系）
    if let Ok(output) = Command::new("apt-cache")
        .arg("policy")
        .arg("linglong-bin")
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("Installed:") || line.contains("已安装：") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if let Some(v) = parts.get(1) {
                        result.ll_bin_version = Some(v.trim().to_string());
                    }
                }
            }
        }
    }

    // 版本校验：版本过低时仅警告，不阻止使用
    if let Some(ref v) = result.ll_version {
        if compare_versions(v, min_version) == std::cmp::Ordering::Less {
            // 不设置 ok=false，允许用户继续使用
            result.reason = Some(format!(
                "当前玲珑基础环境版本({})过低，建议升级至 >= {}",
                v, min_version
            ));
            // 注意：这里不再 return，继续后续检查
        }
    } else {
        result.ok = false;
        result.reason = Some("无法检测到玲珑环境版本，请确认已安装".to_string());
        return Ok(result);
    }

    // 检测容器环境变量
    result.is_container = std::env::var("LINYAPS_CONTAINER")
        .map(|v| v == "yes")
        .unwrap_or(false);

    result.ok = true;
    Ok(result)
}

pub async fn install_linglong_env(script_content: String) -> Result<InstallLinglongResult, String> {
    if script_content.trim().is_empty() {
        return Err("安装脚本内容为空".to_string());
    }
    let script = script_content.clone();
    let handle = tokio::task::spawn_blocking(move || -> Result<InstallLinglongResult, String> {
        let file_name = format!(
            "install-linglong-{}.sh",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("获取时间失败: {}", e))?
                .as_millis()
        );
        let mut path = PathBuf::from(std::env::temp_dir());
        path.push(file_name);

        {
            let mut file = fs::File::create(&path)
                .map_err(|e| format!("创建安装脚本失败: {}", e))?;
            file.write_all(script.as_bytes())
                .map_err(|e| format!("写入安装脚本失败: {}", e))?;
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置脚本权限失败: {}", e))?;

        info!("[install_linglong_env] executing script at {:?}", path);
        let output = Command::new("pkexec")
            .arg("bash")
            .arg(&path)
            .output()
            .map_err(|e| format!("执行安装脚本失败: {}", e))?;

        if !output.status.success() {
            warn!(
                "[install_linglong_env] script failed with code {:?}",
                output.status.code()
            );
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(format!(
                "安装失败(code {:?}): {}",
                output.status.code(),
                stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // 清理脚本文件
        let _ = fs::remove_file(&path);
        Ok(InstallLinglongResult { stdout, stderr })
    });

    handle
        .await
        .map_err(|e| format!("安装任务执行失败: {}", e))?
}
