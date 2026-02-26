import { useCallback, useEffect, type RefObject } from 'react'

interface UseAutoLoadWhenNotScrollableOptions {
  containerRef: RefObject<HTMLDivElement>
  loading: boolean
  hasMore: boolean
  onLoadMore: () => void
  threshold?: number
  deps?: unknown[]
}

export const useAutoLoadWhenNotScrollable = ({
  containerRef,
  loading,
  hasMore,
  onLoadMore,
  threshold = 100,
  deps = [],
}: UseAutoLoadWhenNotScrollableOptions) => {
  const tryLoadWhenNotScrollable = useCallback(() => {
    const listElement = containerRef.current
    if (!listElement || loading || !hasMore) {
      return
    }

    const notScrollable = listElement.scrollHeight <= listElement.clientHeight + 1
    if (notScrollable) {
      onLoadMore()
    }
  }, [containerRef, loading, hasMore, onLoadMore])

  useEffect(() => {
    const handleScroll = () => {
      if (loading || !hasMore) {
        return
      }

      const listElement = containerRef.current
      if (!listElement) {
        return
      }

      const { scrollTop, scrollHeight, clientHeight } = listElement
      if (scrollTop + clientHeight >= scrollHeight - threshold) {
        onLoadMore()
      }
    }

    const listElement = containerRef.current
    if (!listElement) {
      return
    }

    listElement.addEventListener('scroll', handleScroll)

    return () => {
      listElement.removeEventListener('scroll', handleScroll)
    }
  }, [containerRef, loading, hasMore, onLoadMore, threshold])

  useEffect(() => {
    const rafId = window.requestAnimationFrame(() => {
      tryLoadWhenNotScrollable()
    })

    return () => {
      window.cancelAnimationFrame(rafId)
    }
  }, [tryLoadWhenNotScrollable, ...deps])

  useEffect(() => {
    const listElement = containerRef.current
    if (!listElement || typeof ResizeObserver === 'undefined') {
      return
    }

    const observer = new ResizeObserver(() => {
      tryLoadWhenNotScrollable()
    })

    observer.observe(listElement)

    return () => {
      observer.disconnect()
    }
  }, [containerRef, tryLoadWhenNotScrollable])
}
