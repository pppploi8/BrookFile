import * as pdfjsLib from 'pdfjs-dist'
import PdfWorker from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { reactive } from 'vue'
import { removeBookmarkSynced, toggleBookmarkSynced } from '../persistence'
import type {
  EngineOptions,
  PageLayout,
  PageTurnMode,
  ReaderAction,
  ReaderBookmark,
  ReaderEngine,
  ReaderState,
  TocItem,
} from '../types'
import type { PDFDocumentProxy, PDFPageProxy } from 'pdfjs-dist'

// 在模块加载时设置 PDF.js 的 worker 入口（Vite 会将其作为静态资源处理）。
pdfjsLib.GlobalWorkerOptions.workerSrc = PdfWorker

const VIEWPORT_PAD = 12 // 与 .reader-viewport 的 padding 保持一致
const SPREAD_GAP = 16 // 双页对开时两页之间的间距

interface ViewportSize {
  w: number
  h: number
}

// 双指缩放结束后的滚动锚定参数（见 applyAnchoredScroll）。
interface PinchAnchor {
  r: number // 最终缩放比例（z1 / 手势起点 zoom）
  si: number // 锚点所在 spread
  oy: number // 锚点在 spread 内的纵向偏移（缩放前）
  ox: number // 锚点在页内的横向偏移（缩放前）
  pageK: number // 锚点所在页在 spread 内的序号
  relx0: number // 锚点相对首页左缘的横向偏移（兜底用）
  mx: number // 手势结束时中点的视口坐标
  my: number
  padTop: number // contentEl 相对滚动内容原点的偏移（视口 padding）
  padLeft: number
}

// PDF 渲染引擎：基于 pdfjs-dist。
// 支持两种翻页模式（滚动 / 翻页）与两种版面（单页 / 双页对开），
// 仅实现 pdf 的渲染，其余格式由各自的引擎实现，外壳保持不变。
export class PdfEngine implements ReaderEngine {
  readonly format = 'pdf' as const
  readonly state: ReaderState = reactive({
    loading: true,
    ready: false,
    error: null,
    progress: 0,
    locationLabel: '',
    canPrev: false,
    canNext: false,
    toc: [],
    bookmarks: [],
    bookmarked: false,
    total: 0,
    turnMode: 'scroll',
    layout: 'single',
  })
  readonly actions: ReaderAction[]

  private opts: EngineOptions
  private pdfDoc: PDFDocumentProxy | null = null
  private scrollEl: HTMLElement | null = null
  private contentEl: HTMLElement | null = null
  private observer: IntersectionObserver | null = null
  private resizeObserver: ResizeObserver | null = null
  private lastViewportW = 0
  private resizeTimer = 0

  private turnMode: PageTurnMode = 'scroll'
  private layout: PageLayout = 'single'
  private zoom = 1

  // 未渲染 spread 的占位高度估计（用已渲染 spread 的实际高度推断），
  // 避免空 spread 在 flex 容器里塌陷成 0x0 导致 IntersectionObserver 永不触发。
  private placeholderH = 0
  // 全部页面的原始尺寸（scale=1），用于渲染前计算准确的 spread 高度，
  // 防止占位高度与真实高度差异过大导致恢复/滚动位置漂移。
  private pageDims: ViewportSize[] = []
  // 已渲染过的 spread，避免观察器/滚动/缩放重复渲染同一屏（缩放用 force 绕过）。
  private renderedSpreads = new Set<number>()
  // 每个 spread 的渲染令牌：并发渲染同一 spread 时旧结果作废，避免画布重复叠加。
  private renderTokens = new Map<number, number>()

  // 每个 spread 是一屏要显示的页（单页=1 个页码，双页=2 个页码）。
  private spreads: number[][] = []
  private spreadEls: HTMLElement[] = []
  private currentSpread = 0
  private currentPage = 1

  private scrollRaf = 0
  private destroyed = false

  // ---- 触摸手势（移动端）：双指缩放 + 放大后单指横向平移 ----
  // touch-action: pan-y 保留原生纵向滚动，横向与双指手势交给这里处理。
  private touchPinch: {
    zoom0: number
    dist0: number
    ax: number // 锚点：手势起点中点在 contentEl 本地坐标系的位置
    ay: number
    m0x: number // 手势起点中点（相对 scrollEl 可视区左上角）
    m0y: number
    k: number // 当前缩放比例（相对 zoom0，已按上下限收敛）
    mex: number // 最近一次中点位置
    mey: number
    si: number
    oy: number
    ox: number
    pageK: number
    relx0: number
    padTop: number
    padLeft: number
  } | null = null
  private touchPan: { startX: number; startY: number; startScrollLeft: number; active: boolean; decided: boolean } | null = null

  constructor(opts: EngineOptions) {
    this.opts = opts
    this.actions = [
      { key: 'zoom-out', icon: 'ZoomOut', title: 'reader.zoomOut', run: () => this.setZoom(this.zoom / 1.2) },
      { key: 'zoom-in', icon: 'ZoomIn', title: 'reader.zoomIn', run: () => this.setZoom(this.zoom * 1.2) },
      { key: 'fit-width', icon: 'Aim', title: 'reader.fitWidth', run: () => this.setZoom(1) },
    ]
  }

  async mount(container: HTMLElement): Promise<void> {
    this.scrollEl = container
    container.addEventListener('scroll', this.onScroll, { passive: true })
    // pan-y：纵向滚动交给浏览器，横向平移与双指缩放由触摸手势处理
    container.style.touchAction = 'pan-y'
    container.addEventListener('touchstart', this.onTouchStart, { passive: true })
    container.addEventListener('touchmove', this.onTouchMove, { passive: false })
    container.addEventListener('touchend', this.onTouchEnd, { passive: true })
    container.addEventListener('touchcancel', this.onTouchEnd, { passive: true })
    this.enableResizeObserver()
    this.contentEl = document.createElement('div')
    this.contentEl.className = 'pdf-content'
    container.appendChild(this.contentEl)

    try {
      const buf = await this.opts.getSource()
      if (this.destroyed) return
      const task = pdfjsLib.getDocument({ data: new Uint8Array(buf) })
      this.pdfDoc = await task.promise
      if (this.destroyed || !this.pdfDoc) return

      this.state.total = this.pdfDoc.numPages
      await this.loadPageDims()
      if (this.destroyed || !this.pdfDoc) return
      this.buildSpreads()
      this.buildDom()
      this.applyMode()

      this.state.loading = false
      this.state.ready = true
      this.updateCurrentSpread()
      this.updateNav()
      // 占位尺寸先就位（让懒加载观察器能正确触发），首屏立即渲染
      this.setPlaceholders()
      void this.renderSpread(this.currentSpread, true)
    } catch (e: unknown) {
      this.state.loading = false
      this.state.error = e instanceof Error ? e.message : 'PDF_LOAD_FAILED'
    }
  }

  // ---- 版面 / 翻页模式 ----

  // 加载全部页的原始尺寸（ getPage 只解析页字典，不渲染，代价很小）。
  private async loadPageDims(): Promise<void> {
    if (!this.pdfDoc) return
    const total = this.pdfDoc.numPages
    const results = await Promise.all(
      Array.from({ length: total }, (_, i) =>
        this.pdfDoc!.getPage(i + 1).then((pg) => {
          const v = pg.getViewport({ scale: 1 })
          return { w: v.width, h: v.height } as ViewportSize
        }).catch(() => null),
      ),
    )
    if (this.destroyed) return
    this.pageDims = results.map((r) => r ?? { w: 0, h: 0 })
  }

  // 根据页尺寸计算未渲染 spread 在当前缩放/版面下的预期高度（无尺寸数据返回 0）。
  private expectedSpreadHeight(i: number): number {
    const spread = this.spreads[i]
    if (!spread || this.pageDims.length === 0) return 0
    const bases: ViewportSize[] = []
    for (const p of spread) {
      const d = this.pageDims[p - 1]
      if (d && d.w > 0) bases.push(d)
    }
    if (bases.length === 0) return 0
    const scale = this.computeScale(bases)
    return Math.max(...bases.map((b) => b.h)) * scale
  }

  private buildSpreads(): void {
    this.spreads = []
    const n = this.state.total
    if (this.layout === 'double') {
      for (let p = 1; p <= n; p += 2) {
        const pair = [p]
        if (p + 1 <= n) pair.push(p + 1)
        this.spreads.push(pair)
      }
    } else {
      for (let p = 1; p <= n; p++) this.spreads.push([p])
    }
  }

  private buildDom(): void {
    if (!this.contentEl) return
    this.spreadEls = []
    this.renderedSpreads.clear()
    this.renderTokens.clear()
    this.placeholderH = 0
    this.contentEl.innerHTML = ''
    for (let i = 0; i < this.spreads.length; i++) {
      const se = document.createElement('div')
      se.className = 'pdf-spread' + (this.layout === 'double' ? ' double' : '')
      se.dataset.spread = String(i)
      // 双页明确横向排布（左右对开），避免任何 CSS 歧义导致上下堆叠
      se.style.display = 'flex'
      se.style.flexDirection = 'row'
      se.style.flexWrap = 'nowrap'
      se.style.justifyContent = 'center'
      se.style.alignItems = 'flex-start'
      se.style.gap = `${SPREAD_GAP}px`
      se.style.width = '100%'
      // 占满交叉轴宽度，避免空 spread 在 align-items:center 下塌缩为 0 宽
      se.style.alignSelf = 'stretch'
      this.contentEl.appendChild(se)
      this.spreadEls.push(se)
    }
    // 先给未渲染的 spread 一个占位高度，IntersectionObserver 才能正确触发
    this.setPlaceholders()
  }

  // 为尚未渲染的 spread 设置占位 min-height，使懒加载观察器可触发、滚动高度正确。
  // 已有页尺寸时按页尺寸精确计算；否则用已渲染 spread 的实测高度推断；
  // 无基准时用视口高度兜底。
  private setPlaceholders(): void {
    if (!this.scrollEl) return
    let h = this.placeholderH
    for (const el of this.spreadEls) {
      if (el.querySelector('.pdf-page') && el.offsetHeight > h) h = el.offsetHeight
    }
    if (h <= 0) h = Math.max(400, this.scrollEl.clientHeight)
    this.placeholderH = h
    for (let i = 0; i < this.spreadEls.length; i++) {
      const el = this.spreadEls[i]
      if (!el || el.querySelector('.pdf-page')) continue
      const eh = this.expectedSpreadHeight(i)
      el.style.minHeight = eh > 0 ? `${Math.ceil(eh)}px` : `${h}px`
    }
  }

  // 根据当前模式应用容器样式与可见性，并决定懒渲染策略。
  private applyMode(): void {
    if (!this.scrollEl || !this.contentEl) return
    if (this.turnMode === 'scroll') {
      this.scrollEl.style.overflow = 'auto'
      this.contentEl.style.display = 'flex'
      this.contentEl.style.flexDirection = 'column'
      this.contentEl.style.alignItems = 'center'
      this.contentEl.style.justifyContent = 'flex-start'
      this.contentEl.style.gap = `${VIEWPORT_PAD}px`
      this.contentEl.style.width = '100%'
      this.contentEl.style.height = 'auto'
      this.spreadEls.forEach((el) => (el.style.display = 'flex'))
      this.enableObserver()
    } else {
      // 翻页模式：一屏仅显示当前 spread；纵向滚动只看当前 spread 的溢出，
      // 滚动到底不会进入下一页，需通过翻页（上一页/下一页）进入下一屏。
      this.scrollEl.style.overflow = 'auto'
      this.contentEl.style.display = 'flex'
      this.contentEl.style.flexDirection = 'column'
      this.contentEl.style.alignItems = 'center'
      this.contentEl.style.justifyContent = 'flex-start'
      this.contentEl.style.gap = '0'
      this.contentEl.style.width = '100%'
      this.contentEl.style.height = 'auto'
      this.contentEl.style.minHeight = '100%'
      this.disableObserver()
      this.showOnlyCurrent()
      if (this.scrollEl) this.scrollEl.scrollTop = 0
    }
  }

  setPageTurnMode(mode: PageTurnMode): void {
    if (mode === this.turnMode) return
    this.turnMode = mode
    this.state.turnMode = mode
    this.applyMode()
    if (mode === 'paginated') {
      // 翻页模式的意义是宽屏显示两页（避免一页太浪费），进入时若为单页则自动转双页
      if (this.layout === 'single') {
        this.setPageLayout('double')
      } else {
        this.showOnlyCurrent()
        void this.renderSpread(this.currentSpread, true)
      }
    } else {
      this.enableObserver()
      this.scrollToSpread(this.currentSpread, false)
    }
  }

  setPageLayout(layout: PageLayout): void {
    if (layout === this.layout) return
    this.layout = layout
    this.state.layout = layout
    this.buildSpreads()
    this.buildDom()
    this.currentSpread = this.spreadIndexForPage(this.currentPage)
    this.applyMode()
    if (this.turnMode === 'paginated') {
      this.showOnlyCurrent()
      void this.renderSpread(this.currentSpread, true)
    } else {
      this.enableObserver()
      this.scrollToSpread(this.currentSpread, false)
    }
    this.updateCurrentSpread()
    this.updateNav()
  }

  // ---- 渲染 ----

  // 计算某个 spread 在当前版面/缩放下应该使用的缩放系数。
  // 统一按「宽度」fit：双页时每页取半宽（减间距），单页取满宽。
  // 翻页模式下不按高度 fit，这样放大后超出视口的部份可以纵向滚动查看。
  private computeScale(bases: ViewportSize[]): number {
    const pad = VIEWPORT_PAD * 2
    const availW = (this.scrollEl?.clientWidth || 800) - pad
    const perPageW = this.layout === 'double' ? (availW - SPREAD_GAP) / 2 : availW
    const maxW = bases.reduce((m, b) => Math.max(m, b.w), 0)
    const scale = perPageW / Math.max(maxW, 1)
    return scale * this.zoom
  }

  private async renderSpread(i: number, force = false): Promise<void> {
    if (!force && this.renderedSpreads.has(i)) return
    const spread = this.spreads[i]
    const el = this.spreadEls[i]
    if (!spread || !el || !this.pdfDoc || this.destroyed) return
    this.renderedSpreads.add(i)
    const token = (this.renderTokens.get(i) || 0) + 1
    this.renderTokens.set(i, token)
    if (el.querySelector('.pdf-page')) el.style.minHeight = `${el.offsetHeight}px`
    el.innerHTML = ''
    let pages: PDFPageProxy[]
    try {
      pages = await Promise.all(spread.map((p) => this.pdfDoc!.getPage(p)))
    } catch {
      // 页获取失败时允许观察器稍后重试
      this.renderedSpreads.delete(i)
      return
    }
    if (this.destroyed || this.renderTokens.get(i) !== token) return
    const bases = pages.map((pg) => {
      const v = pg.getViewport({ scale: 1 })
      return { w: v.width, h: v.height }
    })
    const scale = this.computeScale(bases)
    // 页组宽于容器时左对齐：flex 居中溢出的起始部分不可滚动到达，左对齐后才能横向平移到
    const groupW = bases.reduce((s, b) => s + b.w * scale, 0) + SPREAD_GAP * Math.max(0, pages.length - 1)
    const contentW = (this.scrollEl?.clientWidth || 800) - VIEWPORT_PAD * 2
    el.style.justifyContent = groupW > contentW + 1 ? 'flex-start' : 'center'
    const dpr = window.devicePixelRatio || 1
    for (let k = 0; k < spread.length; k++) {
      const pg = pages[k]!
      const viewport = pg.getViewport({ scale })
      const canvas = document.createElement('canvas')
      const ctx = canvas.getContext('2d')
      if (!ctx) continue
      canvas.width = Math.floor(viewport.width * dpr)
      canvas.height = Math.floor(viewport.height * dpr)
      canvas.style.width = `${Math.floor(viewport.width)}px`
      canvas.style.height = `${Math.floor(viewport.height)}px`
      try {
        await pg.render({
          canvasContext: ctx,
          viewport,
          transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
        }).promise
      } catch {
        // 渲染可能被取消（缩放 / 翻页 / 销毁），忽略即可
        continue
      }
      if (this.destroyed || this.renderTokens.get(i) !== token) return
      const wrap = document.createElement('div')
      wrap.className = 'pdf-page'
      // 这些元素是 TypeScript 动态创建的，Vue scoped CSS 不会作用到它们，
      // 所以把关键样式内联，避免双页对开时退化成 block 上下堆叠。
      wrap.style.display = 'flex'
      wrap.style.alignItems = 'center'
      wrap.style.justifyContent = 'center'
      wrap.style.background = '#fff'
      wrap.style.boxShadow = '0 1px 4px rgba(0, 0, 0, 0.15)'
      wrap.style.overflow = 'hidden'
      wrap.style.flexShrink = '0'
      wrap.appendChild(canvas)
      el.appendChild(wrap)
    }
    el.style.minHeight = ''
    this.setPlaceholders()
  }

  private onIntersect = (entries: IntersectionObserverEntry[]): void => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        const i = Number((entry.target as HTMLElement).dataset.spread)
        void this.renderSpread(i)
      }
    }
  }

  private enableResizeObserver(): void {
    if (!this.scrollEl || this.resizeObserver) return
    this.lastViewportW = this.scrollEl.clientWidth
    this.resizeObserver = new ResizeObserver(() => {
      if (!this.scrollEl || this.destroyed) return
      const w = this.scrollEl.clientWidth
      if (w === this.lastViewportW) return
      this.lastViewportW = w
      if (this.resizeTimer) clearTimeout(this.resizeTimer)
      this.resizeTimer = window.setTimeout(() => {
        this.resizeTimer = 0
        this.handleViewportResized()
      }, 200)
    })
    this.resizeObserver.observe(this.scrollEl)
  }

  private handleViewportResized(): void {
    if (this.destroyed || !this.pdfDoc || !this.scrollEl) return
    this.resetDistantSpreads()
    this.rerenderVisible()
    this.setPlaceholders()
  }

  private resetDistantSpreads(): void {
    if (!this.scrollEl) return
    // 窗口与懒加载观察器的 rootMargin(400px) 保持一致，
    // 否则观察器刚预热渲染的 spread 会被这里清掉且不再重渲染。
    const top = this.scrollEl.scrollTop - 400
    const bottom = this.scrollEl.scrollTop + this.scrollEl.clientHeight + 400
    this.spreadEls.forEach((el, i) => {
      if (!el.querySelector('.pdf-page')) return
      const hidden = el.style.display === 'none'
      const off = this.spreadTop(el)
      const h = el.offsetHeight
      if (!hidden && off + h >= top && off <= bottom) return
      this.renderedSpreads.delete(i)
      el.innerHTML = ''
      el.style.minHeight = `${h || this.placeholderH}px`
    })
  }

  private enableObserver(): void {
    if (!this.scrollEl) return
    if (!this.observer) {
      this.observer = new IntersectionObserver(this.onIntersect, {
        root: this.scrollEl,
        rootMargin: '400px 0px',
      })
    }
    this.spreadEls.forEach((el) => this.observer!.observe(el))
  }

  private disableObserver(): void {
    if (this.observer) this.observer.disconnect()
  }

  // ---- 导航 ----

  private spreadIndexForPage(page: number): number {
    for (let i = 0; i < this.spreads.length; i++) {
      if (this.spreads[i]?.includes(page)) return i
    }
    return 0
  }

  private spreadTop(el: HTMLElement): number {
    return el.offsetTop - (this.contentEl?.offsetTop ?? 0)
  }

  private showOnlyCurrent(): void {
    this.spreadEls.forEach((el, idx) => {
      el.style.display = idx === this.currentSpread ? 'flex' : 'none'
    })
  }

  private scrollToSpread(i: number, smooth: boolean): void {
    if (this.spreads.length === 0) return
    const idx = Math.min(Math.max(0, i), this.spreads.length - 1)
    this.currentSpread = idx
    if (this.turnMode === 'paginated') {
      this.showOnlyCurrent()
      if (this.scrollEl) this.scrollEl.scrollTop = 0
      void this.renderSpread(idx)
    } else {
      const el = this.spreadEls[idx]
      if (el && this.scrollEl) {
        this.scrollEl.scrollTo({ top: Math.max(0, this.spreadTop(el) - VIEWPORT_PAD), behavior: smooth ? 'smooth' : 'auto' })
      }
    }
    this.updateCurrentSpread()
    this.updateNav()
  }

  private notifyPosition(jump: boolean): void {
    this.opts.onPositionChange?.(jump)
  }

  goPrev(): void {
    this.scrollToSpread(this.currentSpread - 1, true)
    this.notifyPosition(false)
  }

  goNext(): void {
    this.scrollToSpread(this.currentSpread + 1, true)
    this.notifyPosition(false)
  }

  // 当前页内容超出视口部分已滚动的比例（页内滚动比例，整页可见时为 0）。
  private inPageRatio(): number {
    const el = this.spreadEls[this.currentSpread]
    if (!el || !this.scrollEl) return 0
    const over = el.offsetHeight - this.scrollEl.clientHeight
    if (over <= 0) return 0
    const top = this.spreadTop(el) - VIEWPORT_PAD
    const scrolled = this.scrollEl.scrollTop - top
    return Math.min(1, Math.max(0, scrolled / over))
  }

  // 内容坐标："页码:页内滚动比例"（比例保留 4 位小数）。
  getContentCoord(): string | null {
    if (this.state.total === 0) return null
    return `${this.currentPage}:${this.inPageRatio().toFixed(4)}`
  }

  // 待应用的目标页内滚动比例（goToContentCoord 定位到页后异步应用）。
  private pendingRatio: number | null = null

  // 解析 "页码[:页内比例]" 或单独页码并定位（不发位置事件）。
  private navigateToCoord(input: string, smooth: boolean): boolean {
    const s = input.trim()
    if (!s) return false
    const sep = s.indexOf(':')
    const n = parseInt(sep >= 0 ? s.slice(0, sep) : s, 10)
    if (isNaN(n) || n < 1 || n > this.state.total) return false
    const ratio = sep >= 0 ? parseFloat(s.slice(sep + 1)) : NaN
    this.pendingRatio = isNaN(ratio) ? null : Math.min(1, Math.max(0, ratio))
    const idx = this.spreadIndexForPage(n)
    this.scrollToSpread(idx, smooth)
    // 目标 spread 可能距当前很远，观察器之外也直接渲染，确保 applyPendingRatio 能等到它
    void this.renderSpread(idx)
    this.applyPendingRatio()
    return true
  }

  goToLocation(input: string): boolean {
    const ok = this.navigateToCoord(input, true)
    if (ok) this.notifyPosition(true)
    return ok
  }

  goToContentCoord(coord: string): boolean {
    return this.navigateToCoord(coord, false)
  }

  // 等目标 spread 渲染完成后重新锚定到该 spread 的页内位置（最多等待约 30 秒，
  // 大文件首屏渲染可能较慢；渲染过程中占位高度被替换为真实高度会导致滚动位置漂移，
  // 因此无论比例是否为 0 都要重新锚定）。
  private applyPendingRatio(): void {
    const ratio = this.pendingRatio
    if (ratio === null) return
    const targetSpread = this.currentSpread
    const deadline = Date.now() + 30000
    const tryScroll = (): void => {
      if (this.destroyed) return
      // 用户已手动离开目标 spread 时放弃锚定
      if (this.currentSpread !== targetSpread) {
        this.pendingRatio = null
        return
      }
      const el = this.spreadEls[targetSpread]
      if (el && el.querySelector('.pdf-page') && this.scrollEl) {
        this.pendingRatio = null
        const over = el.offsetHeight - this.scrollEl.clientHeight
        const off = over > 0 ? Math.round(Math.min(over, ratio * over)) : 0
        this.scrollEl.scrollTop = Math.max(0, this.spreadTop(el) - VIEWPORT_PAD + off)
        return
      }
      if (Date.now() < deadline) requestAnimationFrame(tryScroll)
      else this.pendingRatio = null
    }
    requestAnimationFrame(tryScroll)
  }

  goToToc(_item: TocItem): void {
    // PDF 大纲（索引）暂未实现，预留接口
  }

  // 渲染任意页为 JPEG dataURL（AI 页面查看工具用，独立于阅读画布）。
  // 输出宽度收敛到 1000px，控制截图体积。
  async renderPageImage(page: number): Promise<string | null> {
    if (!this.pdfDoc || page < 1 || page > this.state.total) return null
    try {
      const pg = await this.pdfDoc.getPage(page)
      const base = pg.getViewport({ scale: 1 })
      const scale = Math.min(2, Math.max(0.5, 1000 / base.width))
      const viewport = pg.getViewport({ scale })
      const canvas = document.createElement('canvas')
      canvas.width = Math.floor(viewport.width)
      canvas.height = Math.floor(viewport.height)
      const ctx = canvas.getContext('2d')
      if (!ctx) return null
      ctx.fillStyle = '#fff'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      await pg.render({ canvasContext: ctx, viewport }).promise
      return canvas.toDataURL('image/jpeg', 0.85)
    } catch {
      return null
    }
  }

  // ---- 书签 ----

  // 书签按页码比较（坐标 "页码:比例" 的页码部分）。
  toggleBookmark(): void {
    const first = this.spreads[this.currentSpread]?.[0]
    if (first === undefined) return
    toggleBookmarkSynced({
      state: this.state,
      opts: this.opts,
      refresh: () => this.updateBookmarkedFlag(),
      coord: this.getContentCoord(),
      label: `P.${first}`,
      matches: (b, coord) => parseInt(String(b.location), 10) === parseInt(coord, 10),
    })
  }

  removeBookmark(id: string): void {
    removeBookmarkSynced(
      { state: this.state, opts: this.opts, refresh: () => this.updateBookmarkedFlag() },
      id,
    )
  }

  goToBookmark(item: ReaderBookmark): void {
    if (this.navigateToCoord(String(item.location), true)) this.notifyPosition(true)
  }

  private updateBookmarkedFlag(): void {
    const first = this.spreads[this.currentSpread]?.[0]
    this.state.bookmarked =
      first !== undefined &&
      this.state.bookmarks.some((b) => parseInt(String(b.location), 10) === first)
  }

  private onScroll = (): void => {
    if (this.turnMode !== 'scroll') return
    if (this.scrollRaf) return
    this.scrollRaf = requestAnimationFrame(() => {
      this.scrollRaf = 0
      this.computeCurrentFromScroll()
    })
  }

  private computeCurrentFromScroll(): void {
    if (!this.scrollEl) return
    const top = this.scrollEl.scrollTop + this.scrollEl.clientHeight / 2
    let cur = 0
    for (let i = 0; i < this.spreadEls.length; i++) {
      const el = this.spreadEls[i]
      if (!el) break
      if (this.spreadTop(el) <= top) cur = i
      else break
    }
    if (cur !== this.currentSpread) {
      this.currentSpread = cur
      this.updateCurrentSpread()
      this.updateNav()
    }
    // 页内滚动比例也在变化，统一上报（保存时实时读取当前比例）
    this.notifyPosition(false)
  }

  private updateCurrentSpread(): void {
    const spread = this.spreads[this.currentSpread]
    if (!spread) return
    const first = spread[0]!
    const last = spread[spread.length - 1]!
    this.currentPage = first
    this.state.locationLabel =
      spread.length > 1 ? `${first}-${last} / ${this.state.total}` : `${first} / ${this.state.total}`
    this.state.progress = this.state.total > 0 ? (first - 1) / this.state.total : 0
    this.updateBookmarkedFlag()
  }

  private updateNav(): void {
    const last = this.spreads.length - 1
    this.state.canPrev = this.currentSpread > 0
    this.state.canNext = this.currentSpread < last
  }

  private setZoom(z: number, anchor?: PinchAnchor): void {
    this.zoom = Math.min(3, Math.max(0.3, z))
    this.resetDistantSpreads()
    this.rerenderVisible()
    // 缩放改变了未渲染 spread 的预期高度，同步刷新占位高度
    this.setPlaceholders()
    if (anchor) {
      this.applyAnchoredScroll(anchor)
    } else {
      requestAnimationFrame(() => {
        if (this.turnMode === 'scroll') this.scrollToSpread(this.currentSpread, false)
      })
    }
  }

  // ---- 触摸手势（移动端） ----

  private onTouchStart = (e: TouchEvent): void => {
    if (this.destroyed || !this.scrollEl || !this.contentEl) return
    // 双指变成其它数量时结束进行中的缩放手势
    if (this.touchPinch && e.touches.length !== 2) this.finishPinch()
    if (e.touches.length === 2) {
      const [a, b] = [e.touches[0]!, e.touches[1]!]
      this.contentEl.style.transform = ''
      this.contentEl.style.transformOrigin = ''
      const sc = this.scrollEl.getBoundingClientRect()
      const cc = this.contentEl.getBoundingClientRect()
      const mx = (a.clientX + b.clientX) / 2
      const my = (a.clientY + b.clientY) / 2
      const ax = mx - cc.left
      const ay = my - cc.top
      // 锚点所在 spread / 页及其内偏移
      let si = this.currentSpread
      for (let i = 0; i < this.spreadEls.length; i++) {
        const el = this.spreadEls[i]!
        const top = this.spreadTop(el)
        if (ay >= top && ay < top + el.offsetHeight) {
          si = i
          break
        }
      }
      const spreadEl = this.spreadEls[si]
      const pages = spreadEl ? [...spreadEl.querySelectorAll<HTMLElement>('.pdf-page')] : []
      let pageK = 0
      if (pages.length > 0) {
        for (let k = 0; k < pages.length; k++) {
          const x = pages[k]!.offsetLeft - this.contentEl.offsetLeft
          if (ax >= x && ax < x + pages[k]!.offsetWidth) {
            pageK = k
            break
          }
        }
      }
      const px = pages[pageK] ? pages[pageK]!.offsetLeft - this.contentEl.offsetLeft : 0
      const px0 = pages[0] ? pages[0].offsetLeft - this.contentEl.offsetLeft : 0
      this.touchPinch = {
        zoom0: this.zoom,
        dist0: Math.max(1, Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY)),
        ax,
        ay,
        m0x: mx - sc.left,
        m0y: my - sc.top,
        k: 1,
        mex: mx - sc.left,
        mey: my - sc.top,
        si,
        oy: spreadEl ? ay - this.spreadTop(spreadEl) : ay,
        ox: ax - px,
        pageK,
        relx0: ax - px0,
        padTop: cc.top - sc.top + this.scrollEl.scrollTop,
        padLeft: cc.left - sc.left + this.scrollEl.scrollLeft,
      }
      this.touchPan = null
    } else if (e.touches.length === 1 && !this.touchPinch) {
      const t0 = e.touches[0]!
      this.touchPan = {
        startX: t0.clientX,
        startY: t0.clientY,
        startScrollLeft: this.scrollEl.scrollLeft,
        active: false,
        decided: false,
      }
    }
  }

  private onTouchMove = (e: TouchEvent): void => {
    if (this.destroyed || !this.scrollEl || !this.contentEl) return
    if (this.touchPinch && e.touches.length === 2) {
      e.preventDefault()
      const [a, b] = [e.touches[0]!, e.touches[1]!]
      const sc = this.scrollEl.getBoundingClientRect()
      const p = this.touchPinch
      const dist = Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY)
      const z1 = Math.min(3, Math.max(0.3, p.zoom0 * (dist / p.dist0)))
      p.k = z1 / p.zoom0
      p.mex = (a.clientX + b.clientX) / 2 - sc.left
      p.mey = (a.clientY + b.clientY) / 2 - sc.top
      // 手势期间用 transform 即时反馈（锚点跟随双指中点），结束后按新 zoom 重渲染
      this.contentEl.style.transformOrigin = `${p.ax}px ${p.ay}px`
      this.contentEl.style.transform = `translate(${p.mex - p.m0x}px, ${p.mey - p.m0y}px) scale(${p.k})`
      return
    }
    const pan = this.touchPan
    if (pan && !this.touchPinch && e.touches.length === 1) {
      const t = e.touches[0]!
      const dx = t.clientX - pan.startX
      const dy = t.clientY - pan.startY
      const overflowX = this.scrollEl.scrollWidth - this.scrollEl.clientWidth > 1
      if (!pan.decided) {
        // 横向占优且存在横向溢出时才接管控拽，否则让浏览器纵向滚动
        if (Math.abs(dx) > 8 && Math.abs(dx) > Math.abs(dy) * 1.2 && overflowX) {
          pan.active = true
          pan.decided = true
        } else if (Math.abs(dy) > 8) {
          pan.decided = true
        }
      }
      if (pan.active) {
        e.preventDefault()
        this.scrollEl.scrollLeft = pan.startScrollLeft - dx
      }
    }
  }

  private onTouchEnd = (e: TouchEvent): void => {
    if (this.destroyed) return
    if (this.touchPinch && e.touches.length < 2) {
      this.finishPinch()
      this.touchPan = null
      return
    }
    if (e.touches.length === 0) {
      this.touchPan = null
    }
  }

  private finishPinch(): void {
    const p = this.touchPinch
    this.touchPinch = null
    if (!p || !this.contentEl || !this.scrollEl) return
    this.contentEl.style.transform = ''
    this.contentEl.style.transformOrigin = ''
    const z1 = Math.min(3, Math.max(0.3, p.zoom0 * p.k))
    this.setZoom(z1, {
      r: z1 / p.zoom0,
      si: p.si,
      oy: p.oy,
      ox: p.ox,
      pageK: p.pageK,
      relx0: p.relx0,
      mx: p.mex,
      my: p.mey,
      padTop: p.padTop,
      padLeft: p.padLeft,
    })
  }

  // 双指缩放结束后按锚点恢复滚动位置：锚点内容坐标随页面等比缩放（ox/oy 按比例），
  // spread 间距与视口 padding 不变；等可见 spread 重渲染完成后再测量实际 offset 设置滚动，
  // 避免渲染中途高度未稳定导致滚动被钳制在旧范围。
  private applyAnchoredScroll(anchor: PinchAnchor): void {
    const deadline = Date.now() + 10000
    const tryApply = (): void => {
      if (this.destroyed || !this.scrollEl || !this.contentEl) return
      const top = this.scrollEl.scrollTop - 400
      const bottom = this.scrollEl.scrollTop + this.scrollEl.clientHeight + 400
      const allRendered = this.spreadEls.every((el) => {
        if (!el || el.style.display === 'none') return true
        const off = this.spreadTop(el)
        const h = el.offsetHeight
        if (off + h >= top && off <= bottom) return !!el.querySelector('.pdf-page')
        return true
      })
      if (!allRendered && Date.now() < deadline) {
        requestAnimationFrame(tryApply)
        return
      }
      const el = this.spreadEls[anchor.si]
      if (el) {
        const ay = this.spreadTop(el) + anchor.oy * anchor.r
        const pages = [...el.querySelectorAll<HTMLElement>('.pdf-page')]
        const pk = pages[anchor.pageK]
        const ax = pk
          ? pk.offsetLeft - this.contentEl.offsetLeft + anchor.ox * anchor.r
          : pages[0]
            ? pages[0].offsetLeft - this.contentEl.offsetLeft + anchor.relx0 * anchor.r
            : anchor.ox * anchor.r
        const maxY = Math.max(0, this.scrollEl.scrollHeight - this.scrollEl.clientHeight)
        const maxX = Math.max(0, this.scrollEl.scrollWidth - this.scrollEl.clientWidth)
        this.scrollEl.scrollTop = Math.min(maxY, Math.max(0, anchor.padTop + ay - anchor.my))
        this.scrollEl.scrollLeft = Math.min(maxX, Math.max(0, anchor.padLeft + ax - anchor.mx))
      }
    }
    requestAnimationFrame(tryApply)
  }

  private rerenderVisible(): void {
    if (!this.scrollEl) return
    if (this.turnMode === 'paginated') {
      void this.renderSpread(this.currentSpread, true)
      return
    }
    const top = this.scrollEl.scrollTop - 400
    const bottom = this.scrollEl.scrollTop + this.scrollEl.clientHeight + 400
    this.spreadEls.forEach((el, i) => {
      if (!el) return
      const off = this.spreadTop(el)
      const h = el.offsetHeight
      if (off + h >= top && off <= bottom) void this.renderSpread(i, true)
    })
  }

  destroy(): void {
    this.destroyed = true
    this.disableObserver()
    this.observer = null
    if (this.resizeObserver) {
      this.resizeObserver.disconnect()
      this.resizeObserver = null
    }
    if (this.resizeTimer) clearTimeout(this.resizeTimer)
    if (this.scrollEl) {
      this.scrollEl.removeEventListener('scroll', this.onScroll)
      this.scrollEl.removeEventListener('touchstart', this.onTouchStart)
      this.scrollEl.removeEventListener('touchmove', this.onTouchMove)
      this.scrollEl.removeEventListener('touchend', this.onTouchEnd)
      this.scrollEl.removeEventListener('touchcancel', this.onTouchEnd)
      this.scrollEl.style.touchAction = ''
    }
    if (this.scrollRaf) cancelAnimationFrame(this.scrollRaf)
    if (this.pdfDoc) {
      void this.pdfDoc.destroy()
      this.pdfDoc = null
    }
    if (this.contentEl?.parentNode) {
      this.contentEl.parentNode.removeChild(this.contentEl)
      this.contentEl = null
    }
  }
}
