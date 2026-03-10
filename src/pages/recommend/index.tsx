import AppCarousel from '@/components/ApplicationCarousel'
import ApplicationCard from '@/components/ApplicationCard'
import ApplicationCardSkeleton from '@/components/ApplicationCardSkeleton'
import styles from './index.module.scss'
import { getWelcomeCarouselList, getWelcomeAppList } from '@/apis/apps/index'
import { useCallback, useEffect, useState, useRef } from 'react'
import { useGlobalStore } from '@/stores/global'
import { usePaginatedList } from '@/hooks/usePaginatedList'
import { useApplicationCardModel } from '@/hooks/useApplicationCardModel'

type AppInfo = API.APP.AppMainDto
const defaultPageSize = 10 // 每页显示数量

const Recommend = () => {
  const arch = useGlobalStore((state) => state.arch)
  const repoName = useGlobalStore((state) => state.repoName)
  const { getCardState, handleInstall, uninstall } = useApplicationCardModel()

  const [carouselList, setCarouselList] = useState<AppInfo[]>([])
  const listRef = useRef<HTMLDivElement>(null)

  const fetcher = useCallback(async(pageNo: number) => {
    const res = await getWelcomeAppList({ repoName, arch, pageNo, pageSize: defaultPageSize })
    return { records: (res.data.records || []) as AppInfo[], pages: res.data.pages || 1 }
  }, [repoName, arch])

  const {
    items: recommendList,
    loading,
    initialLoading,
    hasMore,
    loadPage,
  } = usePaginatedList<AppInfo>({
    fetcher,
    containerRef: listRef as React.RefObject<HTMLDivElement>,
    pageSize: defaultPageSize,
  })

  // 首屏：并行获取轮播图 + 第一页推荐
  useEffect(() => {
    const fetchCarousel = async() => {
      try {
        const result = await getWelcomeCarouselList({ repoName, arch })
        if (result.code === 200 && result.data?.length > 0) {
          setCarouselList(result.data as AppInfo[])
        }
      } catch (error) {
        console.error('Failed to fetch carousel data:', error)
      }
    }

    fetchCarousel()
    loadPage(1, true)
  }, [repoName, arch, loadPage])

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
                {!loading && !hasMore && recommendList.length > 0 && <div className={styles.noMoreTip}>没有更多数据了</div>}
              </>
            )}
          </div>
        </div>
      </main>
    </div>
  )
}

export default Recommend
