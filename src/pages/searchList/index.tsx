import styles from './index.module.scss'
import ApplicationCard from '@/components/ApplicationCard'
import { useGlobalStore, useSearchStore } from '@/stores/global'
import { getSearchAppList } from '@/apis/apps/index'
import { useState, useEffect, useRef, useCallback } from 'react'
import { generateEmptyCards } from './utils'
import { Empty } from 'antd'
import { useAutoLoadWhenNotScrollable } from '@/hooks/useAutoLoadWhenNotScrollable'
const defaultPageSize = 10 // 每页显示数量

type AppInfo = API.APP.AppMainDto
const SearchList = ()=>{
  const keyword = useSearchStore((state) => state.keyword)
  const arch = useGlobalStore((state) => state.arch)
  const repoName = useGlobalStore((state) => state.repoName)
  const [pageNo, setPageNo] = useState<number>(1)
  const [loading, setLoading] = useState<boolean>(false)
  const [totalPages, setTotalPages] = useState<number>(1)
  const [searchAppList, setSearchAppList] = useState<AppInfo[]>([])
  const listRef = useRef<HTMLDivElement>(null)

  // 获取应用列表
  const getSearchKeyAppList = useCallback(async({ pageNo = 1, init = false }: { pageNo?: number, init?: boolean }) => {
    if (!keyword) {
      return
    }

    setLoading(true)
    if (init) {
      // 初始化时先显示空卡片占位
      setSearchAppList(generateEmptyCards(defaultPageSize))
    }
    try {
      const res = await getSearchAppList({
        name: keyword,
        repoName,
        arch,
        pageNo,
        pageSize: defaultPageSize,
      })

      const newRecords = res.data.records || []

      if (init) {
        // 初始化时直接替换
        setSearchAppList(newRecords)
      } else {
        // 追加新数据时，过滤掉空卡片后再追加
        setSearchAppList(prev => {
          const filteredPrev = prev.filter(item => !item.appId?.startsWith('empty-'))
          return [...filteredPrev, ...newRecords]
        })
      }

      setTotalPages(res.data.pages || 1)
      setPageNo(pageNo)
    } catch (error) {
      console.error('获取应用列表失败:', error)
      // 错误时移除空卡片
      if (init) {
        setSearchAppList([])
      }
    } finally {
      setLoading(false)
    }
  }, [keyword, repoName, arch])

  const loadNextPage = useCallback(() => {
    if (loading || pageNo >= totalPages) {
      return
    }
    getSearchKeyAppList({ pageNo: pageNo + 1 })
  }, [loading, pageNo, totalPages, getSearchKeyAppList])

  // 初始化获取数据
  useEffect(() => {
    if (!keyword) {
      setSearchAppList([])
      setPageNo(1)
      setTotalPages(1)
      return
    }

    setPageNo(1)
    setTotalPages(1)
    getSearchKeyAppList({ pageNo: 1, init: true })
  }, [keyword, getSearchKeyAppList])

  useAutoLoadWhenNotScrollable({
    containerRef: listRef,
    loading,
    hasMore: pageNo < totalPages,
    onLoadMore: loadNextPage,
    deps: [searchAppList, keyword, pageNo, totalPages],
  })

  return <div className={styles.searchPage} ref={listRef}>
    <p className={styles.SearchResult}>搜索结果：</p>
    <div className={searchAppList.length > 0 ? styles.SearchList : styles.SearchListEmpty}>
      {
        searchAppList.length > 0 ? searchAppList.map((item, index) => {
          return (
            <ApplicationCard
              key={`${item.appId}_${index}`}
              appInfo={item}
              operateId={1}
            />
          )
        }) : <Empty description="没有搜索到数据哦！"/>
      }
      {loading && <div className={styles.loadingTip}>加载中...</div>}
      {!loading && totalPages <= pageNo && searchAppList.length > 0 && <div className={styles.noMoreTip}>没有更多数据了</div>}
    </div>
  </div>
}
export default SearchList
