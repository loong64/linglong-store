/**
 * 统一分页列表状态机
 * 封装 pageNo / totalPages / loading / initialLoading / items 以及自动补页逻辑
 * 页面只需提供 fetcher 函数和容器 ref
 */
import { useState, useCallback, type RefObject } from 'react'
import { useAutoLoadWhenNotScrollable } from './useAutoLoadWhenNotScrollable'

/** fetcher 返回的分页结果 */
export interface PaginatedResult<T> {
  records: T[]
  /** 总页数 */
  pages: number
}

interface UsePaginatedListOptions<T> {
  /** 数据获取函数，由页面提供，接收页码返回分页结果 */
  fetcher: (pageNo: number) => Promise<PaginatedResult<T>>
  /** 滚动容器 ref，传给 useAutoLoadWhenNotScrollable */
  containerRef: RefObject<HTMLDivElement>
  /** 每页大小（仅用于骨架屏 count 等，不影响 fetcher） */
  pageSize?: number
  /** useAutoLoadWhenNotScrollable 的额外依赖 */
  extraDeps?: unknown[]
}

export function usePaginatedList<T>({
  fetcher,
  containerRef,
  pageSize = 10,
  extraDeps = [],
}: UsePaginatedListOptions<T>) {
  const [items, setItems] = useState<T[]>([])
  const [pageNo, setPageNo] = useState(1)
  const [totalPages, setTotalPages] = useState(1)
  const [loading, setLoading] = useState(false)
  const [initialLoading, setInitialLoading] = useState(true)

  const hasMore = pageNo < totalPages

  /**
   * 加载指定页
   * @param page 页码
   * @param init 是否为首屏（重置列表）
   */
  const loadPage = useCallback(async(page: number, init = false) => {
    if (loading) {
      return
    }

    setLoading(true)
    if (init) {
      setInitialLoading(true)
      setItems([])
    }

    try {
      const result = await fetcher(page)
      const newRecords = result.records ?? []

      if (init) {
        setItems(newRecords)
      } else {
        setItems(prev => [...prev, ...newRecords])
      }

      setTotalPages(result.pages || 1)
      setPageNo(page)
    } catch (error) {
      console.error('[usePaginatedList] 加载失败:', error)
      if (init) {
        setItems([])
      }
    } finally {
      if (init) {
        setInitialLoading(false)
      }
      setLoading(false)
    }
  }, [fetcher, loading])

  /** 加载下一页 */
  const loadNextPage = useCallback(() => {
    if (loading || !hasMore) {
      return
    }
    loadPage(pageNo + 1)
  }, [loading, hasMore, pageNo, loadPage])

  /** 重置到初始状态（不触发请求） */
  const reset = useCallback(() => {
    setItems([])
    setPageNo(1)
    setTotalPages(1)
    setInitialLoading(true)
  }, [])

  // 自动补页 + 滚动触底加载
  useAutoLoadWhenNotScrollable({
    containerRef,
    loading,
    hasMore,
    onLoadMore: loadNextPage,
    deps: [items, pageNo, totalPages, ...extraDeps],
  })

  return {
    items,
    pageNo,
    totalPages,
    loading,
    initialLoading,
    hasMore,
    loadPage,
    loadNextPage,
    reset,
    pageSize,
  }
}
