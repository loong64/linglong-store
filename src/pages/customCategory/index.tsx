import styles from './index.module.scss'
import ApplicationCard from '@/components/ApplicationCard'
import { useEffect, useState, useRef, useCallback } from 'react'
import { getAppListByCategoryIds, getRecommendAppList } from '@/apis/apps/index'
import { useGlobalStore } from '@/stores/global'
import { OperateType } from '@/constants/applicationCard'
import { Select, Checkbox, Spin, Empty, type CheckboxProps } from 'antd'
import { useParams } from 'react-router-dom'
import { useAutoLoadWhenNotScrollable } from '@/hooks/useAutoLoadWhenNotScrollable'
const defaultPageSize = 30 // 每页显示数量
type AppInfo = API.APP.AppMainDto
const OfficeApps = () => {
  const { arch, repoName, customMenuCategory } = useGlobalStore()
  const [pageNo, setPageNo] = useState<number>(1)
  const [loading, setLoading] = useState<boolean>(false)
  const [initialLoading, setInitialLoading] = useState<boolean>(true)
  const [totalPages, setTotalPages] = useState<number>(1)
  const [allAppList, setAllAppList] = useState<AppInfo[]>([])
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
      pageSize: 5, // 只获取3个，可以写死
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

  // 获取应用列表
  const getAllAppList = useCallback(async({
    pageNo = 1,
    init = false,
    filterValue,
    sortValue,
  }: {
    pageNo?: number
    init?: boolean
    filterValue: boolean
    sortValue: string
  }) => {
    setLoading(true)

    if (init) {
      setInitialLoading(true)
      setAllAppList([])
    }
    try {
      const res = await getAppListByCategoryIds({
        menuCode: code,
        repoName,
        arch,
        filter: filterValue,
        sortType: sortValue,
        pageNo,
        pageSize: defaultPageSize,
      })

      const newRecords = res.data.records || []

      if (init) {
        // 初始化时直接替换
        setAllAppList(newRecords)
      } else {
        // 追加新数据时，过滤掉空卡片后再追加
        setAllAppList(prev => {
          const filteredPrev = prev.filter(item => !item.appId?.startsWith('empty-'))
          return [...filteredPrev, ...newRecords]
        })
      }

      setTotalPages(res.data.pages || 1)
      setPageNo(pageNo)
    } catch (error) {
      console.error('获取应用列表失败:', error)
      if (init) {
        setAllAppList([])
      }
    } finally {
      if (init) {
        setInitialLoading(false)
      }
      setLoading(false)
    }
  }, [code, repoName, arch])

  const loadNextPage = useCallback(() => {
    if (loading || pageNo >= totalPages) {
      return
    }
    void getAllAppList({
      pageNo: pageNo + 1,
      filterValue: filter,
      sortValue: sortType,
    })
  }, [loading, pageNo, totalPages, getAllAppList, filter, sortType])

  // 初始化获取数据
  useEffect(() => {
    setPageNo(1)
    setTotalPages(1)
    void getAllAppList({
      pageNo: 1,
      init: true,
      filterValue: filter,
      sortValue: sortType,
    })
    void getHeaderRecommendAppList()
  }, [code, getAllAppList, getHeaderRecommendAppList])
  // 监听filter和sortType参数变化
  useEffect(() => {
    if (skipFilterSortEffectRef.current) {
      skipFilterSortEffectRef.current = false
      return
    }
    setPageNo(1)
    setTotalPages(1)
    void getAllAppList({
      pageNo: 1,
      init: true,
      filterValue: filter,
      sortValue: sortType,
    })
  }, [filter, sortType, getAllAppList])

  useAutoLoadWhenNotScrollable({
    containerRef: listRef,
    loading,
    hasMore: pageNo < totalPages,
    onLoadMore: loadNextPage,
    deps: [allAppList, pageNo, totalPages],
  })

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
          return index < 3 && (
            <ApplicationCard
              type="recommend"
              key={`${item.appId}_${index}`}
              appInfo={item}
              operateId={OperateType.INSTALL}
            />
          )
        })
      }
    </div>
    <div className={styles.applicationList}>
      {initialLoading ? (
        <div className={styles.initialLoading}>
          <Spin size="large" tip="加载中..." />
        </div>
      ) : allAppList.length > 0 ? (
        <>
          {
            allAppList.map((item, index) => {
              return (
                <ApplicationCard
                  key={`${item.appId}_${index}`}
                  appInfo={item}
                  operateId={OperateType.INSTALL}
                />
              )
            })
          }
          {loading && <div className={styles.loadingTip}>加载中...</div>}
          {!loading && totalPages <= pageNo && <div className={styles.noMoreTip}>没有更多数据了</div>}
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
