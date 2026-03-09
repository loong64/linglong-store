/**
 * 安装服务封装层
 * 封装底层 Tauri invoke 调用，提供统一的错误处理和类型安全
 */
import { invoke } from '@tauri-apps/api/core'

/**
 * 安装错误类型
 */
export type InstallErrorType =
  | 'network' // 网络错误
  | 'not_found' // 应用不存在
  | 'permission' // 权限不足
  | 'disk_space' // 磁盘空间不足
  | 'dependency' // 依赖问题
  | 'force_required' // 需要强制安装
  | 'unknown' // 未知错误

/**
 * 安装错误
 */
export interface InstallError {
  type: InstallErrorType
  message: string
  originalError?: unknown
}

/**
 * 解析错误消息，分类错误类型
 */
const classifyError = (error: unknown): InstallError => {
  const message = error instanceof Error ? error.message : String(error)
  const lowerMessage = message.toLowerCase()

  // 检测是否需要 force 安装
  if (message.includes('ll-cli install') && message.includes('--force')) {
    return {
      type: 'force_required',
      message: '该版本已安装，需要使用强制安装模式',
      originalError: error,
    }
  }

  // 网络错误
  if (
    lowerMessage.includes('network') ||
    lowerMessage.includes('connection') ||
    lowerMessage.includes('timeout') ||
    lowerMessage.includes('fetch')
  ) {
    return {
      type: 'network',
      message: '网络连接失败，请检查网络设置',
      originalError: error,
    }
  }

  // 应用不存在
  if (
    lowerMessage.includes('not found') ||
    lowerMessage.includes('no such') ||
    lowerMessage.includes('does not exist')
  ) {
    return {
      type: 'not_found',
      message: '应用不存在或版本不可用',
      originalError: error,
    }
  }

  // 权限问题
  if (
    lowerMessage.includes('permission') ||
    lowerMessage.includes('access denied') ||
    lowerMessage.includes('privilege')
  ) {
    return {
      type: 'permission',
      message: '权限不足，请检查系统权限设置',
      originalError: error,
    }
  }

  // 磁盘空间
  if (
    lowerMessage.includes('disk') ||
    lowerMessage.includes('space') ||
    lowerMessage.includes('storage')
  ) {
    return {
      type: 'disk_space',
      message: '磁盘空间不足，请清理后重试',
      originalError: error,
    }
  }

  // 依赖问题
  if (
    lowerMessage.includes('dependency') ||
    lowerMessage.includes('runtime') ||
    lowerMessage.includes('require')
  ) {
    return {
      type: 'dependency',
      message: '缺少依赖或运行时环境',
      originalError: error,
    }
  }

  // 未知错误
  return {
    type: 'unknown',
    message,
    originalError: error,
  }
}

/**
 * 执行应用安装
 * @param appId - 应用ID
 * @param version - 可选的版本号
 * @param force - 是否强制安装
 * @returns Promise<string> 安装结果
 * @throws InstallError 安装失败时抛出
 */
export const executeInstall = async(
  appId: string,
  version?: string,
  force = false,
): Promise<string> => {
  try {
    console.info(`[installService] Executing install: appId=${appId}, version=${version}, force=${force}`)

    const result = await invoke<string>('install_app', {
      appId,
      version: version || null,
      force,
    })

    console.info(`[installService] Install completed: ${appId}`)
    return result
  } catch (error) {
    const classifiedError = classifyError(error)
    console.error(`[installService] Install failed: ${appId}`, classifiedError)
    throw classifiedError
  }
}

/**
 * 检查是否需要 force 安装
 * @param errorMessage - 错误消息
 */
export const isForceRequired = (errorMessage: string): boolean => {
  if (!errorMessage) {
    return false
  }
  const normalized = errorMessage.replace(/\s+/g, ' ')
  return normalized.includes('ll-cli install') && normalized.includes('--force')
}
