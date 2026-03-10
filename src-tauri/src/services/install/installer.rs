//! 安装器核心逻辑
//!
//! 本模块实现玲珑应用安装的核心逻辑，包括：
//! - 安装流程管理
//! - 进程生命周期管理
//! - 取消操作处理
//!
//! 状态机模型（基于 ll-cli-json-install-gui-requirements.md）：
//! - IDLE -> WAITING: 启动安装
//! - WAITING -> INSTALLING: 收到进度百分比
//! - INSTALLING/WAITING -> FAILED: 收到错误/超时/进程异常退出
//! - INSTALLING -> SUCCEEDED: 进程正常退出 (exit code 0)

use log::{error, info, warn};
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::services::ll_cli_command;

use super::json_parser::{JsonEventType, JsonLineParser};
use super::progress_emitter::{ProgressEmitter, ThreadSafeProgressEmitter};
use super::slot::InstallSlot;
use super::state_machine::InstallStateMachine;

/// 安装指定的玲珑应用
///
/// # Arguments
/// * `app_handle` - Tauri 应用句柄，用于发送进度事件
/// * `app_id` - 应用 ID（例如：org.deepin.calculator）
/// * `version` - 可选的版本号（如果为空，则安装最新版本）
/// * `force` - 是否强制安装
///
/// # Returns
/// * `Ok(String)` - 安装成功消息
/// * `Err(String)` - 安装失败原因
pub async fn install_linglong_app(
    app_handle: AppHandle,
    app_id: String,
    version: Option<String>,
    force: bool,
) -> Result<String, String> {
    info!("========== [Installer] START ==========");
    info!("[Installer] app_id: {}", app_id);
    info!("[Installer] version: {:?}", version);
    info!("[Installer] force: {}", force);

    // 1. 尝试占用安装槽位
    InstallSlot::acquire(&app_id)?;

    // 创建进度发送器
    let emitter = ProgressEmitter::new(&app_handle, app_id.clone());

    // 2. 构建应用引用
    let app_ref = if let Some(ref ver) = version {
        format!("{}/{}", app_id, ver)
    } else {
        app_id.clone()
    };

    // 3. 构建命令
    let mut cmd = ll_cli_command();
    cmd.arg("install")
        .arg(&app_ref)
        .arg("--json")
        .arg("-y");

    if force {
        cmd.arg("--force");
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let command_str = format!(
        "ll-cli install {} --json -y{}",
        app_ref,
        if force { " --force" } else { "" }
    );
    info!("[Installer] Executing: {}", command_str);

    // 4. 启动子进程
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let err_msg = format!("Failed to spawn ll-cli process: {}", e);
            error!("[Installer] ERROR: {}", err_msg);
            InstallSlot::release();
            return Err(err_msg);
        }
    };

    info!("[Installer] Process spawned successfully");

    // 5. 获取 stdout
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let err_msg = "Failed to capture stdout".to_string();
            error!("[Installer] ERROR: {}", err_msg);
            InstallSlot::release();
            return Err(err_msg);
        }
    };

    // 6. 初始化状态机
    let state_machine = Arc::new(Mutex::new(InstallStateMachine::new()));

    // 启动状态机
    if let Ok(mut sm) = state_machine.lock() {
        sm.start();
    }

    // 存储 child 以便后续等待
    let child_arc = Arc::new(Mutex::new(child));

    // 7. 发送初始等待事件
    emitter.emit_waiting();

    // 8. 在子线程中读取 stdout
    let thread_emitter = ThreadSafeProgressEmitter::new(app_handle.clone(), app_id.clone());
    let state_machine_clone = state_machine.clone();
    let last_error: Arc<Mutex<Option<(i32, String)>>> = Arc::new(Mutex::new(None));
    let last_error_clone = last_error.clone();

    let reader_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut last_percentage: u32 = 0;

        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(e) => {
                    warn!("[Installer:Reader] Error reading line: {}", e);
                    continue;
                }
            };

            info!("[Installer:Reader] Raw line: {}", line);

            // 解析 JSON 行
            let event = match JsonLineParser::parse(&line) {
                Some(e) => e,
                None => continue,
            };

            info!("[Installer:Reader] Parsed event: {:?}", event);

            match event.event_type {
                JsonEventType::Progress => {
                    // 更新状态机
                    if let Ok(mut sm) = state_machine_clone.lock() {
                        sm.on_progress(event.percentage.unwrap_or(0.0));
                    }

                    let percentage = (event.percentage.unwrap_or(0.0) as u32).min(100);

                    // 只有百分比变化时才发送
                    if percentage != last_percentage {
                        last_percentage = percentage;
                        thread_emitter.emit_progress(percentage, &event.message);
                    }
                }
                JsonEventType::Error => {
                    // 更新状态机
                    if let Ok(mut sm) = state_machine_clone.lock() {
                        sm.on_error();
                    }

                    let code = event.code.unwrap_or(-1);

                    // 保存错误信息
                    if let Ok(mut last_err) = last_error_clone.lock() {
                        *last_err = Some((code, event.message.clone()));
                    }

                    thread_emitter.emit_error(code, &event.message);
                }
                JsonEventType::Message => {
                    // 刷新状态机时间戳
                    if let Ok(mut sm) = state_machine_clone.lock() {
                        sm.touch();
                    }
                    thread_emitter.emit_message(&event.message, last_percentage);
                }
            }
        }

        info!("[Installer:Reader] Finished reading stdout");
    });

    info!("[Installer] Waiting for process to complete...");

    // 9. 轮询等待进程结束，同时检查超时
    let exit_status = loop {
        // 检查进程状态
        let status = {
            let mut child = match child_arc.lock() {
                Ok(c) => c,
                Err(e) => {
                    let err_msg = format!("Failed to lock child process: {}", e);
                    error!("[Installer] ERROR: {}", err_msg);
                    InstallSlot::release();
                    return Err(err_msg);
                }
            };

            match child.try_wait() {
                Ok(Some(status)) => {
                    info!("[Installer] Process exited: {:?}", status);
                    Some(status)
                }
                Ok(None) => None,
                Err(e) => {
                    let err_msg = format!("Failed to check process status: {}", e);
                    error!("[Installer] ERROR: {}", err_msg);
                    InstallSlot::release();
                    return Err(err_msg);
                }
            }
        };

        // 检查超时
        {
            let sm = match state_machine.lock() {
                Ok(sm) => sm,
                Err(e) => {
                    let err_msg = format!("Lock error: {}", e);
                    InstallSlot::release();
                    return Err(err_msg);
                }
            };

            if sm.check_timeout() {
                warn!("[Installer] Progress timeout. Killing process...");

                // 终止进程
                if let Ok(mut child) = child_arc.lock() {
                    let _ = child.kill();
                }

                // 发送超时事件
                emitter.emit_timeout();

                // 释放槽位
                InstallSlot::release();

                return Err("Installation timed out".to_string());
            }
        }

        if let Some(status) = status {
            break status;
        }

        // 短暂休眠
        std::thread::sleep(std::time::Duration::from_millis(100));
    };

    // 10. 等待读取线程完成
    let _ = reader_handle.join();

    info!("==========================================================");
    info!("[Installer] Process exited with status: {:?}", exit_status);

    // 11. 检查是否被用户取消
    let was_cancelled = InstallSlot::is_cancelled();

    // 12. 释放槽位（统一由安装方法释放）
    InstallSlot::release();

    // 13. 根据退出状态和取消标志判断结果
    if exit_status.success() {
        // 安装成功
        if let Ok(mut sm) = state_machine.lock() {
            sm.on_success();
        }

        let success_msg = if let Some(ver) = version {
            format!("Successfully installed {} version {}", app_id, ver)
        } else {
            format!("Successfully installed {}", app_id)
        };

        info!("[Installer] SUCCESS: {}", success_msg);
        emitter.emit_success();

        info!("========== [Installer] END ==========");
        Ok(success_msg)
    } else if was_cancelled {
        // 用户取消导致的退出，不发送失败消息（取消方法已发送）
        info!("[Installer] Process killed by user cancellation, skipping error event");
        info!("========== [Installer] END ==========");

        // 返回特殊的取消消息
        Err("Installation cancelled by user".to_string())
    } else {
        // 真正的安装失败
        if let Ok(mut sm) = state_machine.lock() {
            sm.on_failure();
        }

        let (error_code, error_message) = if let Ok(last_err) = last_error.lock() {
            last_err.clone().unwrap_or((-1, "Unknown error".to_string()))
        } else {
            (-1, "Unknown error".to_string())
        };

        let failure_msg = format!("Installation failed: {}", error_message);

        error!("[Installer] FAILED: {}", failure_msg);
        emitter.emit_error(error_code, &error_message);

        info!("========== [Installer] END ==========");
        Err(failure_msg)
    }
}

/// 取消正在进行的安装
///
/// 使用优雅终止方式（SIGTERM）杀死 ll-package-manager 和 ll-cli 进程。
///
/// # Arguments
/// * `app_handle` - Tauri 应用句柄，用于发送取消事件
/// * `app_id` - 应用 ID（保留参数以兼容前端调用）
///
/// # Returns
/// * `Ok(String)` - 取消成功消息
/// * `Err(String)` - 取消失败原因
pub async fn cancel_linglong_install(
    app_handle: AppHandle,
    app_id: String,
) -> Result<String, String> {
    info!("[Installer:Cancel] Cancelling installation for: {}", app_id);

    // 1. 先标记取消状态
    if !InstallSlot::mark_cancelled() {
        let err_msg = "没有正在进行的安装任务".to_string();
        warn!("[Installer:Cancel] {}", err_msg);
        return Err(err_msg);
    }

    // 2. 杀死安装进程
    info!("[Installer:Cancel] Executing: pkexec killall -15 ll-package-manager ll-cli");

    let _output = tokio::process::Command::new("pkexec")
        .arg("killall")
        .arg("-15") // SIGTERM 优雅终止
        .arg("ll-cli")
        .output()
        .await;

    // 3. 发送取消事件
    let emitter = ProgressEmitter::new(&app_handle, app_id);
    emitter.emit_cancelled();

    // 注意：不在这里释放槽位，由 install_linglong_app 统一释放

    let success_msg = "已取消安装".to_string();
    info!("[Installer:Cancel] {}", success_msg);
    Ok(success_msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_integration() {
        // 确保槽位初始为空
        InstallSlot::release();
        assert!(InstallSlot::is_idle());

        // 模拟占用
        InstallSlot::acquire("test.app").unwrap();
        assert!(!InstallSlot::is_idle());

        // 模拟取消
        assert!(InstallSlot::mark_cancelled());
        assert!(InstallSlot::is_cancelled());

        // 释放
        InstallSlot::release();
        assert!(InstallSlot::is_idle());
    }
}
