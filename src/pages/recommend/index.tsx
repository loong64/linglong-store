import AppCarousel from '@/components/ApplicationCarousel'
import ApplicationCard from '@/components/ApplicationCard'
import ApplicationCardSkeleton from '@/components/ApplicationCardSkeleton'
import styles from './index.module.scss'
import { getWelcomeCarouselList, getWelcomeAppList } from '@/apis/apps/index'
import { useCallback, useEffect, useState, useRef } from 'react'
import { useGlobalStore } from '@/stores/global'
import { useAutoLoadWhenNotScrollable } from '@/hooks/useAutoLoadWhenNotScrollable'
import { useApplicationCardModel } from '@/hooks/useApplicationCardModel'

type AppInfo = API.APP.AppMainDto
const defaultPageSize = 10 // 每页显示数量

const Recommend = () => {
  const arch = useGlobalStore((state) => state.arch)
  const repoName = useGlobalStore((state) => state.repoName)
  const { getCardState, handleInstall, uninstall } = useApplicationCardModel()

  const [carouselList, setCarouselList] = useState<AppInfo[]>([])
  const [recommendList, setRecommendList] = useState<AppInfo[]>([])
  const listRef = useRef<HTMLDivElement>(null)
  const [pageNo, setPageNo] = useState<number>(1)
  const [loading, setLoading] = useState<boolean>(false)
  const [initialLoading, setInitialLoading] = useState<boolean>(true)
  const [totalPages, setTotalPages] = useState<number>(1)

  const fetchData = useCallback(async() => {
    try {
      setInitialLoading(true)
      // 并行请求，提高性能
      const [carouselResult, recommendResult] = await Promise.all([
        getWelcomeCarouselList({ repoName, arch }),
        getWelcomeAppList({ repoName, arch, pageNo: 1, pageSize: defaultPageSize }),
      ])

      // 更新轮播图数据
      if (carouselResult.code === 200 && carouselResult.data?.length > 0) {
        setCarouselList(carouselResult.data as AppInfo[])
      }

      // 更新推荐列表数据
      if (recommendResult.code === 200 && recommendResult.data?.records?.length > 0) {
        setRecommendList(recommendResult.data.records as AppInfo[])
        setTotalPages(recommendResult.data.pages || 1)
      }
    } catch (error) {
      console.error('Failed to fetch recommend data:', error)
    } finally {
      setInitialLoading(false)
    }
  }, [repoName, arch])
  // 获取推荐数据函数
  const getWelcomeAppListNext = useCallback(async({ pageNo = 1 }: { pageNo?: number }) => {
    if (loading) {
      return
    }
    setLoading(true)
    try {
      const res = await getWelcomeAppList({
        repoName,
        arch,
        pageNo,
        pageSize: defaultPageSize,
      })

      const newRecords = res.data.records || []
      // 追加新数据
      setRecommendList(prev => [...prev, ...newRecords])

      setTotalPages(res.data.pages || 1)
      setPageNo(pageNo)
    } catch (error) {
      console.error('获取应用列表失败:', error)
    } finally {
      setLoading(false)
    }
  }, [repoName, arch, loading])

  const loadNextPage = useCallback(() => {
    if (loading || pageNo >= totalPages) {
      return
    }
    getWelcomeAppListNext({ pageNo: pageNo + 1 })
  }, [loading, pageNo, totalPages, getWelcomeAppListNext])

  useEffect(() => {
    fetchData()
  }, [fetchData])

  useAutoLoadWhenNotScrollable({
    containerRef: listRef,
    loading,
    hasMore: pageNo < totalPages,
    onLoadMore: loadNextPage,
    deps: [recommendList, pageNo, totalPages],
  })

  return (
    <div className={styles.recommend} ref={listRef} >
      <header className={styles.recommendHead}>
        <AppCarousel carouselList={carouselList} />
      </header>
      <main className={styles.recommendMain}>
        {/* <div className={styles.tabBtn}>
          <Button shape='round' type='default' className={styles.btn}>
            全部应用
          </Button>
        </div> */}
        <div className={styles.appMain}>
          <p className={styles.name}>玲珑推荐</p>
          <div className={styles.appList}>
            {initialLoading ? (
              <ApplicationCardSkeleton count={defaultPageSize} />
            ) : (
              <>
                {recommendList.map((item, index) => {
                  const cardState = getCardState(item)
                  return (
                    <ApplicationCard
                      key={`${item.appId}_${index}`}
                      appInfo={item}
                      operateId={1}
                      isInstalled={cardState.isInstalled}
                      hasUpdate={cardState.hasUpdate}
                      isInstalling={cardState.isInstalling}
                      onInstall={handleInstall}
                      onUninstall={uninstall}
                    />
                  )
                })}
                {loading && <div className={styles.loadingTip}>加载中...</div>}
                {!loading && totalPages <= pageNo && recommendList.length > 0 && <div className={styles.noMoreTip}>没有更多数据了</div>}
              </>
            )}
          </div>
        </div>
      </main>
    </div>
  )
}

export default Recommend
