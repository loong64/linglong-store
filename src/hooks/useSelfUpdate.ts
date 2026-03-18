/**
 * 容器内无感自动更新 Hook
 *
 * 监听 Rust 后端发来的 `self-update-ready` 事件：
 * - 若用户在设置中关闭了自动更新开关，静默忽略（文件已替换，下次启动即生效）
 * - 若开关打开，弹出确认对话框询问是否立即重启
 * - 用户确认重启时，若有安装任务进行中，等待任务全部完成后再退出
 */
import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Modal, message } from 'antd'
import { useConfigStore } from '@/stores/appConfig'
import { useInstallQueueStore } from '@/stores/installQueue'
import { quitApp } from '@/apis/invoke'

export function useSelfUpdate() {
  // 通过 ref 持有最新的 autoSelfUpdate 值，避免事件回调闭包捕获过期值
  const autoSelfUpdate = useConfigStore((state) => state.autoSelfUpdate)
  const autoSelfUpdateRef = useRef(autoSelfUpdate)

  useEffect(() => {
    autoSelfUpdateRef.current = autoSelfUpdate
  }, [autoSelfUpdate])

  useEffect(() => {
    const unlistenPromise = listen<string>('self-update-ready', (event) => {
      const newVersion = event.payload

      // 用户关闭了自动更新提示，静默处理（文件已替换，下次启动即用新版本）
      if (!autoSelfUpdateRef.current) {
        return
      }

      Modal.confirm({
        title: '发现新版本',
        content: `商店已在后台更新至 ${newVersion}，是否立即重启以使用新版本？`,
        okText: '立即重启',
        cancelText: '稍后重启',
        onOk: () => handleRestart(),
      })
    })

    return () => {
      // 组件卸载时取消监听
      unlistenPromise.then((unlisten) => unlisten())
    }
  // 只在挂载时注册一次监听器
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
}

/**
 * 执行重启
 *
 * 若当前有安装任务，等待队列清空后再退出，避免中断安装过程。
 */
function handleRestart() {
  const { isProcessing, currentTask, queue } = useInstallQueueStore.getState()
  const isBusy = isProcessing || !!currentTask || queue.length > 0

  if (!isBusy) {
    quitApp().catch((err) => console.error('[useSelfUpdate] quitApp failed:', err))
    return
  }

  // 有安装任务进行中，展示加载提示，等待队列清空
  const hideLoading = message.loading('正在等待安装任务完成，完成后将自动重启...', 0)

  const unsubscribe = useInstallQueueStore.subscribe((state) => {
    const stillBusy = state.isProcessing || !!state.currentTask || state.queue.length > 0
    if (!stillBusy) {
      unsubscribe()
      hideLoading()
      quitApp().catch((err) => console.error('[useSelfUpdate] quitApp after wait failed:', err))
    }
  })
}
