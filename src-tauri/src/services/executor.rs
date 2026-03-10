//! 统一的 ll-cli 命令执行器
//!
//! 封装命令执行、退出码检查、stdout/stderr 提取，减少各 service 的重复模板代码。
//! 预留 timeout / tracing 扩展点。

use log::{info, warn};
use std::process::Output;

use super::ll_cli_command;

/// 命令执行结果
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub status_code: Option<i32>,
}

/// 执行 ll-cli 命令并统一处理输出
///
/// # Arguments
/// * `args` - 命令参数列表
/// * `label` - 日志标签，用于定位调用来源
///
/// # Returns
/// * `Ok(CliOutput)` - 命令执行完成（无论成功与否）
/// * `Err(String)` - 命令无法启动（如 ll-cli 不存在）
pub fn execute(args: &[&str], label: &str) -> Result<CliOutput, String> {
    let mut cmd = ll_cli_command();
    for arg in args {
        cmd.arg(arg);
    }

    info!("[ll-cli:{}] executing: ll-cli {}", label, args.join(" "));

    let output: Output = cmd
        .output()
        .map_err(|e| format!("[ll-cli:{}] 启动失败: {}", label, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && !stderr.is_empty() {
        warn!("[ll-cli:{}] stderr: {}", label, stderr.trim());
    }

    Ok(CliOutput {
        stdout,
        stderr,
        success: output.status.success(),
        status_code: output.status.code(),
    })
}

/// 执行 ll-cli 命令，失败时直接返回 Err
///
/// 适用于大多数场景：命令执行失败 = 业务失败
///
/// # Arguments
/// * `args` - 命令参数列表
/// * `label` - 日志标签
///
/// # Returns
/// * `Ok(String)` - stdout 内容
/// * `Err(String)` - 命令启动失败或退出码非零时的 stderr
pub fn execute_or_err(args: &[&str], label: &str) -> Result<String, String> {
    let output = execute(args, label)?;

    if !output.success {
        let err_msg = if output.stderr.trim().is_empty() {
            format!("ll-cli {} 执行失败 (code={:?})", args.join(" "), output.status_code)
        } else {
            output.stderr.trim().to_string()
        };
        return Err(err_msg);
    }

    Ok(output.stdout)
}
