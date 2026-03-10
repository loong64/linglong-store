import styles from './index.module.scss'
import ApplicationCard from '@/components/ApplicationCard'
import ApplicationCardSkeleton from '@/components/ApplicationCardSkeleton'
import { useEffect, useState, useRef, useCallback } from 'react'
import { getAppListByCategoryIds, getRecommendAppList } from '@/apis/apps/index'
import { useGlobalStore } from '@/stores/global'
import { OperateType } from '@/constants/applicationCard'
import { Select, Checkbox, Empty, type CheckboxProps } from 'antd'
import { useParams } from 'react-router-dom'
import { usePaginatedList } from '@/hooks/usePaginatedList'
import { useApplicationCardModel } from '@/hooks/useApplicationCardModel'
const defaultPageSize = 30 // 每页显示数量
type AppInfo = API.APP.AppMainDto
const OfficeApps = () => {
  const { arch, repoName, customMenuCategory } = useGlobalStore()
  const { getCardState, handleInstall, uninstall } = useApplicationCardModel()
  const [recommendAppList, setRecommendAppList] = useState<AppInfo[]>([])
  const listRef = useRef<HTMLDivElement>(null)
  const skipFilterSortEffectRef = useRef(true)
  const { code } = useParams()
  const currentCategory = customMenuCategory.find(item => item.code === code)
  const categoryIds = currentCategory?.categoryIds ?? []
  const name = currentCategory?.name ?? ''
  const [filter, setFilter] = useState<boolean>(false) // 是否过滤低分应用
  const [sortType, setSortType] = useState<string>('createTime') // 排序类型

  // 处理过滤低分应用的change事件
  const handleFilterChange:CheckboxProps['onChange'] = (e) => {
    setFilter(e.target.checked)
  }
  // 处理排序类型的change事件
  const handleSortTypeChange = (value: string) => {
    setSortType(value)
  }

  // 获取推荐应用
  const getHeaderRecommendAppList = useCallback(async() => {
    const params = {
      repoName,
      arch,
      pageNo: 1,
      pageSize: 5,
      categoryId: categoryIds.join(','),
    }
    try {
      const res = await getRecommendAppList(params)
      setRecommendAppList(res.data || [])
    } catch (error) {
      console.error('获取推荐应用列表失败:', error)
      setRecommendAppList([])
    }
  }, [repoName, arch, categoryIds])

  const fetcher = useCallback(async(pageNo: number) => {
    const res = await getAppListByCategoryIds({
      menuCode: code,
      repoName,
      arch,
      filter,
      sortType,
      pageNo,
      pageSize: defaultPageSize,
    })
    return { records: (res.data.records || []) as AppInfo[], pages: res.data.pages || 1 }
  }, [code, repoName, arch, filter, sortType])

  const {
    items: allAppList,
    loading,
    initialLoading,
    hasMore,
    loadPage,
  } = usePaginatedList<AppInfo>({
    fetcher,
    containerRef: listRef as React.RefObject<HTMLDivElement>,
    pageSize: defaultPageSize,
  })

  // 初始化获取数据
  useEffect(() => {
    loadPage(1, true)
    getHeaderRecommendAppList().catch(() => undefined)
  }, [code, loadPage, getHeaderRecommendAppList])

  // 监听 filter 和 sortType 参数变化
  useEffect(() => {
    if (skipFilterSortEffectRef.current) {
      skipFilterSortEffectRef.current = false
      return
    }
    loadPage(1, true)
  }, [filter, sortType, loadPage])

  return <div className={styles.officeAppsPage} ref={listRef} >
    <div className={styles.search} >
      <h3>{name}</h3>
      <div className={styles.searchBox}>
        <Select
          defaultValue={sortType}
          style={{ minWidth: '5rem', maxWidth: '20rem', flex: 1 }}
          onChange={handleSortTypeChange}
          options={[
            { value: 'createTime', label: '按上架时间排序' },
            { value: 'installCount', label: '按安装量排序' },
            { value: 'last30Downloads', label: '按近30天下载量排序' },
          ]}
        />
        <Checkbox checked={filter}
          onChange={handleFilterChange}>过滤低分应用</Checkbox>
      </div>
    </div>
    <div className={styles.recommendApplicationList} style={{ marginTop: !initialLoading && recommendAppList.length > 0 ? '3rem' : 0 }}>
      {
        !initialLoading && recommendAppList.map((item, index) => {
          const cardState = getCardState(item)
          return index < 3 && (
            <ApplicationCard
              type="recommend"
              key={`${item.appId}_${index}`}
              appInfo={item}
              operateId={OperateType.INSTALL}
              isInstalled={cardState.isInstalled}
              hasUpdate={cardState.hasUpdate}
              isInstalling={cardState.isInstalling}
              onInstall={handleInstall}
              onUninstall={uninstall}
            />
          )
        })
      }
    </div>
    <div className={styles.applicationList}>
      {initialLoading ? (
        <ApplicationCardSkeleton count={defaultPageSize} />
      ) : allAppList.length > 0 ? (
        <>
          {
            allAppList.map((item, index) => {
              const cardState = getCardState(item)
              return (
                <ApplicationCard
                  key={`${item.appId}_${index}`}
                  appInfo={item}
                  operateId={OperateType.INSTALL}
                  isInstalled={cardState.isInstalled}
                  hasUpdate={cardState.hasUpdate}
                  isInstalling={cardState.isInstalling}
                  onInstall={handleInstall}
                  onUninstall={uninstall}
                />
              )
            })
          }
          {loading && <div className={styles.loadingTip}>加载中...</div>}
          {!loading && !hasMore && <div className={styles.noMoreTip}>没有更多数据了</div>}
        </>
      ) : (
        <div className={styles.emptyState}>
          <Empty description="查无数据" image={null} />
        </div>
      )}
    </div>
  </div>
}

export default OfficeApps
