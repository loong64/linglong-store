import { Tabs } from 'antd'
import ConnectedApplicationCard from '@/components/ConnectedApplicationCard'
import ApplicationCardSkeleton from '@/components/ApplicationCardSkeleton'
import { getNewAppList, getInstallAppList } from '@/apis/apps/index'
import { useGlobalStore } from '@/stores/global'
import styles from './index.module.scss'
import { useEffect, useState, useRef, useCallback } from 'react'
import { usePaginatedList } from '@/hooks/usePaginatedList'

const defaultPageSize = 10 // 每页显示数量

type AppInfo = API.APP.AppMainDto
const Ranking = () => {
  const arch = useGlobalStore((state) => state.arch)
  const repoName = useGlobalStore((state) => state.repoName)
  const [activeTab, setActiveTab] = useState('101')
  const listRef = useRef<HTMLDivElement>(null)

  const fetcher = useCallback(async(pageNo: number) => {
    const params = { repoName, arch, pageNo, pageSize: defaultPageSize }
    const res = activeTab === '102'
      ? await getNewAppList(params)
      : await getInstallAppList(params)
    return { records: (res.data.records || []) as AppInfo[], pages: res.data.pages || 1 }
  }, [repoName, arch, activeTab])

  const {
    items: RankList,
    loading,
    initialLoading,
    hasMore,
    loadPage,
  } = usePaginatedList<AppInfo>({
    fetcher,
    containerRef: listRef as React.RefObject<HTMLDivElement>,
    pageSize: defaultPageSize,
    extraDeps: [activeTab],
  })

  // tab切换
  const handleTabChange = (key: string) => {
    setActiveTab(key)
    // loadPage 会在 fetcher 更新（activeTab 变化）后由 useEffect 触发
  }

  // 初始化 + tab 切换时重新加载
  useEffect(() => {
    loadPage(1, true)
  }, [loadPage])

  return <div className={styles.rankContainer} ref={listRef}>
    <div className={styles.rankHeader}>
      <Tabs defaultActiveKey='101' onChange={handleTabChange} className={styles.customTabs}>
        <Tabs.TabPane tab={ <span style={{ fontSize: '1rem' }}>
            最新上架(前100)
        </span>} key='101' />
        <Tabs.TabPane tab={ <span style={{ fontSize: '1rem' }}>
           下载量(前100)
        </span>} key='202' />
      </Tabs>
    </div>
    <div className={styles.placeholder} />
    <main className={styles.appBox}>
      <div className={styles.appList}>
        {initialLoading ? (
          <ApplicationCardSkeleton count={defaultPageSize} />
        ) : (
          <>
            {RankList.map((item, index) => (
              <ConnectedApplicationCard
                key={`${item.appId}_${index}`}
                appInfo={item}
              />
            ))}
            {loading && <div className={styles.loadingTip}>加载中...</div>}
            {!hasMore && RankList.length > 0 && <div className={styles.noMoreTip}>没有更多数据了</div>}
          </>
        )}
      </div>
    </main>

  </div>
}

export default Ranking
