import { useEffect, useState, useRef } from 'react'
import { Empty } from 'antd'
import styles from './index.module.scss'
import ApplicationCard from '@/components/ApplicationCard'
import { useInstalledAppsStore } from '@/stores/installedApps'
import { useApplicationCardModel } from '@/hooks/useApplicationCardModel'
// import { useConfigStore } from '@/stores/appConfig'
// import { uninstallApp } from '@/apis/invoke'

const MyApplications = () => {
  const {
    installedApps,
    // fetchInstalledApps,
  } = useInstalledAppsStore()
  const { getCardState, handleInstall, uninstall } = useApplicationCardModel()

  // const { showBaseService } = useConfigStore()
  const [mergedApps, setMergedApps] = useState<API.INVOKE.InstalledApp[]>([])
  const listRef = useRef<HTMLDivElement>(null)
  // const [uninstallingAppId, setUninstallingAppId] = useState<string | null>(null)

  useEffect(() => {
    // 合并同appId的应用（显示最新版本，记录版本数）
    if (installedApps.length > 0) {
      const grouped = installedApps.reduce<Record<string, {
        app: API.INVOKE.InstalledApp;
        count: number;
        highestVersion: string;
      }>>((acc, app) => {
        const { appId, version } = app

        if (!acc[appId]) {
          acc[appId] = {
            app,
            count: 1,
            highestVersion: version,
          }
        } else {
          acc[appId].count++
          // 简单的版本比较（可以用更复杂的版本比较库）
          if (compareVersions(version, acc[appId].highestVersion) > 0) {
            acc[appId].highestVersion = version
            acc[appId].app = app
          }
        }

        return acc
      }, {})

      const result = Object.values(grouped).map(({ app, count }) => ({
        ...app,
        occurrenceNumber: count,
      }))

      setMergedApps(result)
    } else {
      setMergedApps([])
    }
  }, [installedApps])

  // 简单的版本比较函数
  const compareVersions = (v1: string, v2: string): number => {
    const parts1 = v1.split('.').map(Number)
    const parts2 = v2.split('.').map(Number)

    for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
      const part1 = parts1[i] || 0
      const part2 = parts2[i] || 0

      if (part1 > part2) {
        return 1
      }
      if (part1 < part2) {
        return -1
      }
    }

    return 0
  }

  // // 处理卸载操作
  // const handleUninstall = (app: API.INVOKE.InstalledApp) => {
  //   Modal.confirm({
  //     title: '确认卸载',
  //     content: `确定要卸载 ${app.zhName || app.name || app.appId} 的版本 ${app.version} 吗？`,
  //     okText: '确定',
  //     cancelText: '取消',
  //     onOk: async() => {
  //       setUninstallingAppId(app.appId)
  //       try {
  //         await uninstallApp(app.appId, app.version)
  //         message.success('卸载成功')

  //         // 重新获取已安装应用列表
  //         await fetchInstalledApps(showBaseService)
  //       } catch (error) {
  //         console.error('[handleUninstall] 卸载失败:', error)
  //         message.error(`卸载失败: ${error}`)
  //       } finally {
  //         setUninstallingAppId(null)
  //       }
  //     },
  //   })
  // }

  return (
    <div className={styles.myAppsPage} ref={listRef}>
      {/* <div className={styles.title}>我的应用</div> */}
      {mergedApps.length > 0 ? <div className={styles.applicationList}>
        {
          mergedApps.map((item, index) => {
            const cardState = getCardState(item)
            return (
              <ApplicationCard
                key={`${item.appId}_${index}`}
                appInfo={item}
                operateId={0}
                isInstalled={cardState.isInstalled}
                hasUpdate={cardState.hasUpdate}
                isInstalling={cardState.isInstalling}
                onInstall={handleInstall}
                onUninstall={uninstall}
              />
            )
          })
        }
      </div> : <Empty description="暂无已安装应用" />}
    </div>
  )
}

export default MyApplications
