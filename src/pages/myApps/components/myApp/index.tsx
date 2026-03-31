import { SearchOutlined } from '@ant-design/icons'
import { useMemo, useState } from 'react'
import { Empty, Input } from 'antd'
import styles from './index.module.scss'
import ConnectedApplicationCard from '@/components/ConnectedApplicationCard'
import { useInstalledAppsStore } from '@/stores/installedApps'
// import { useConfigStore } from '@/stores/appConfig'
// import { uninstallApp } from '@/apis/invoke'

/** 本页 ViewModel：在 EnrichedInstalledApp 基础上附加版本合并数和搜索索引。 */
type MergedApp = API.INVOKE.EnrichedInstalledApp & {
  occurrenceNumber: number
  searchTexts: string[]
}

/**
 * 为本地搜索提取统一的匹配文本。
 * 同一应用不同版本的名称会一并收进搜索索引，避免旧版本别名搜不到。
 */
const collectSearchTexts = (app: API.INVOKE.EnrichedInstalledApp) => [app.zhName, app.name, app.appId]
  .filter((field): field is string => Boolean(field))
  .map(field => field.toLowerCase())

/**
 * 简单的版本比较函数
 * 仅用于“我的应用”页挑选同 appId 的最高版本卡片。
 */
const compareVersions = (v1: string, v2: string): number => {
  const parts1 = v1.split('.').map(Number)
  const parts2 = v2.split('.').map(Number)

  for (let index = 0; index < Math.max(parts1.length, parts2.length); index++) {
    const part1 = parts1[index] || 0
    const part2 = parts2[index] || 0

    if (part1 > part2) {
      return 1
    }
    if (part1 < part2) {
      return -1
    }
  }

  return 0
}

/**
 * 将已安装列表按 appId 合并，只保留最新版本卡片并记录版本个数。
 */
const mergeInstalledApps = (apps: API.INVOKE.EnrichedInstalledApp[]): MergedApp[] => {
  if (apps.length === 0) {
    return []
  }

  const grouped = apps.reduce<Record<string, {
    app: API.INVOKE.EnrichedInstalledApp;
    count: number;
    highestVersion: string;
    searchTexts: string[];
  }>>((accumulator, app) => {
    const { appId, version } = app
    const nextSearchTexts = collectSearchTexts(app)

    if (!accumulator[appId]) {
      accumulator[appId] = {
        app,
        count: 1,
        highestVersion: version,
        searchTexts: nextSearchTexts,
      }
      return accumulator
    }

    accumulator[appId].count += 1
    accumulator[appId].searchTexts = Array.from(new Set([
      ...accumulator[appId].searchTexts,
      ...nextSearchTexts,
    ]))

    if (compareVersions(version, accumulator[appId].highestVersion) > 0) {
      accumulator[appId].highestVersion = version
      accumulator[appId].app = app
    }

    return accumulator
  }, {})

  return Object.values(grouped).map(({ app, count, searchTexts }) => ({
    ...app,
    occurrenceNumber: count,
    searchTexts,
  }))
}

/** 统一清洗搜索词，避免重复 trim / lowerCase。 */
const normalizeKeyword = (value: string) => value.trim().toLowerCase()

/**
 * 匹配“我的应用”页本地搜索关键词。
 * 仅在前端根据已安装 store 做模糊筛选，不触发额外请求。
 */
const matchesKeyword = (app: MergedApp, keyword: string) => {
  if (!keyword) {
    return true
  }

  return app.searchTexts.some(field => field.includes(keyword))
}

const MyApplications = () => {
  const installedApps = useInstalledAppsStore(state => state.installedApps)

  // const { showBaseService } = useConfigStore()
  /** 当前页本地搜索关键词，仅作用于“我的应用”列表。 */
  const [keyword, setKeyword] = useState('')
  // const [uninstallingAppId, setUninstallingAppId] = useState<string | null>(null)

  /** 先合并同 appId 的多版本数据，再交给搜索层进行前端筛选。 */
  const mergedApps = useMemo(() => mergeInstalledApps(installedApps), [installedApps])

  /** 搜索统一做标准化，保证匹配逻辑稳定。 */
  const normalizedKeyword = useMemo(() => normalizeKeyword(keyword), [keyword])

  /** 本地搜索结果：中文名 / 原始名 / appId 模糊匹配。 */
  const filteredApps = useMemo(
    () => mergedApps.filter(app => matchesKeyword(app, normalizedKeyword)),
    [mergedApps, normalizedKeyword],
  )

  /** 区分“没有安装应用”和“搜索无结果”两种空态。 */
  const emptyDescription = installedApps.length === 0
    ? '暂无已安装应用'
    : '未找到匹配的已安装应用'

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
    <div className={styles.myAppsPage}>
      <div className={styles.toolbar}>
        <Input
          allowClear
          className={styles.searchInput}
          placeholder="搜索已安装应用名称或 ID"
          prefix={<SearchOutlined />}
          value={keyword}
          onChange={(event) => setKeyword(event.target.value)}
        />
        <span className={styles.resultMeta}>
          {normalizedKeyword ? `共找到 ${filteredApps.length} 个已安装应用` : `共 ${mergedApps.length} 个已安装应用`}
        </span>
      </div>

      {filteredApps.length > 0 ? <div className={styles.applicationList}>
        {
          filteredApps.map((item) => (
            <ConnectedApplicationCard
              key={item.appId}
              appInfo={item}
              operateId={0}
            />
          ))
        }
      </div> : <div className={styles.emptyState}><Empty description={emptyDescription} /></div>}
    </div>
  )
}

export default MyApplications
