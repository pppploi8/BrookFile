// 阅读器与后端书签/进度接口之间的桥接逻辑，三种格式引擎共用。
// 书签采用乐观更新：先改内存列表，入库失败时回滚（错误提示由请求层统一弹出）。
import type { EngineOptions, ReaderBookmark, ReaderState } from './types'

interface SyncParams {
  state: ReaderState
  opts: EngineOptions
  refresh: () => void
}

// 删除指定书签（乐观更新 + 失败回滚）。
export function removeBookmarkSynced(p: SyncParams, id: string): void {
  const snapshot = p.state.bookmarks.find((b) => b.id === id)
  if (!snapshot) return
  p.state.bookmarks = p.state.bookmarks.filter((b) => b.id !== id)
  p.refresh()
  if (p.opts.removeBookmark) {
    p.opts.removeBookmark(id).catch(() => {
      p.state.bookmarks = [...p.state.bookmarks, snapshot]
      p.refresh()
    })
  }
}

// 在当前位置添加/移除书签。matches 判断已有书签是否与当前坐标相同（命中则移除）。
export function toggleBookmarkSynced(
  p: SyncParams & {
    coord: string | null
    label: string
    matches: (b: ReaderBookmark, coord: string) => boolean
  },
): void {
  if (!p.coord) return
  const existing = p.state.bookmarks.find((b) => p.matches(b, p.coord!))
  if (existing) {
    removeBookmarkSynced(p, existing.id)
    return
  }
  const temp: ReaderBookmark = {
    id: `bm-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    label: p.label,
    location: p.coord,
  }
  p.state.bookmarks = [...p.state.bookmarks, temp]
  p.refresh()
  if (p.opts.addBookmark) {
    p.opts
      .addBookmark(p.coord, p.label)
      .then((id) => {
        temp.id = id
      })
      .catch(() => {
        p.state.bookmarks = p.state.bookmarks.filter((b) => b.id !== temp.id)
        p.refresh()
      })
  }
}

// 3 槽自动进度追踪：
// - 位置变化（滚动/翻页）→ 500ms 防抖后保存到当前槽位；
// - 跳转类动作（目录/书签/跳转输入/最近位置点击）→ 立即轮转指针到下一槽
//   （1→2→3→1，替换最旧槽），再防抖写入新位置；
// - 打开书籍时由外壳加载进度槽：applyLoadedSlots 返回最新槽坐标用于恢复，
//   指针指向最新槽，后续滚动直接刷新该槽。
export class ProgressTracker {
  private slot = 1
  private timer = 0
  private getContentCoord: () => string | null
  private save: (slot: number, coord: string) => Promise<void>

  constructor(
    getContentCoord: () => string | null,
    save: (slot: number, coord: string) => Promise<void>,
  ) {
    this.getContentCoord = getContentCoord
    this.save = save
  }

  // 外壳加载进度槽后调用，返回应恢复的内容坐标（无记录返回 null）。
  applyLoadedSlots(slots: { slot: number; content_coord: string; updated_at: string }[]): string | null {
    if (slots.length === 0) {
      this.slot = 1
      return null
    }
    const latest = slots[0]!
    this.slot = latest.slot
    return latest.content_coord
  }

  onPositionChange(jump: boolean): void {
    if (jump) this.slot = (this.slot % 3) + 1
    this.scheduleSave()
  }

  private scheduleSave(): void {
    if (this.timer) window.clearTimeout(this.timer)
    this.timer = window.setTimeout(() => {
      this.timer = 0
      const coord = this.getContentCoord()
      if (!coord) return
      void this.save(this.slot, coord)
    }, 500)
  }

  dispose(): void {
    if (this.timer) {
      window.clearTimeout(this.timer)
      this.timer = 0
    }
  }
}
