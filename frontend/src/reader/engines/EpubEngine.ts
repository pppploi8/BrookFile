import ePub, { type Book, type Location, type NavItem, type Rendition } from 'epubjs'
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

// EPUB 阅读器设置均以该前缀存入 localStorage。
const LS_PREFIX = 'brookfile.reader.epub.'
const LS_FONT_FAMILY = LS_PREFIX + 'fontFamily'
const LS_FONT_SIZE = LS_PREFIX + 'fontSize'
const LS_THEME = LS_PREFIX + 'theme'
const LS_LINE_SPACING = LS_PREFIX + 'lineSpacing'
const LS_READING_WIDTH = LS_PREFIX + 'readingWidth'

// 可选字体族（第一个为默认）。
const FONT_FAMILIES = [
  'system-ui, sans-serif',
  'Georgia, "Times New Roman", serif',
  '"Microsoft YaHei", "PingFang SC", "Noto Sans CJK SC", sans-serif',
  'monospace',
]

// 预设主题（文字色 + 背景色），工具栏「阅读主题」下拉选择。
const THEMES = [
  { text: '#1f1f1f', bg: '#f7f7f4', label: 'reader.themeLight' },
  { text: '#5b4636', bg: '#f4ecd8', label: 'reader.themeSepia' },
  { text: '#c8c8c8', bg: '#1c1c1c', label: 'reader.themeDark' },
]

// 字体设置下拉的可选字号（px）。
const FONT_SIZES = [14, 16, 18, 20, 22, 24, 28, 32]

const THEME_NAME = 'brookfile'

// EPUB 渲染引擎：基于 epubjs。
// 支持三种可用模式：single+scroll、single+paginated、double+paginated；
// double+scroll 不被 epubjs 支持，切到滚动时自动降级为单页。
// 进度以 CFI（Canonical Fragment Identifier）为内容坐标，位置标签显示「章节 X / 总数」或百分比。
export class EpubEngine implements ReaderEngine {
  readonly format = 'epub' as const
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
  private book: Book | null = null
  private rendition: Rendition | null = null
  private container: HTMLElement | null = null
  private stageEl: HTMLElement | null = null

  private turnMode: PageTurnMode = 'scroll'
  private layout: PageLayout = 'single'

  // 当前阅读位置的 CFI，作为进度与内容坐标。
  private currentCfi: string | null = null
  private spineLength = 0
  private locationsReady = false

  // 模式切换需销毁并重建 rendition，用链式串行化避免并发重建。
  private recreateChain: Promise<void> = Promise.resolve()

  private destroyed = false

  // 阅读设置（构造时从 localStorage 载入）。
  private fontFamily = FONT_FAMILIES[0]!
  private fontSize = 18
  private themeIdx = 0
  private lineSpacing = 1.8
  private readingWidth = 820

  constructor(opts: EngineOptions) {
    this.opts = opts
    this.loadSettings()
    this.actions = [
      { key: 'epub-font', icon: 'Operation', title: 'reader.fontSettings', options: () => FONT_SIZES.map((s) => ({ key: String(s), text: `${s} px`, active: s === this.fontSize })), select: (key) => this.setFontSize(Number(key)) },
      {
        key: 'epub-theme',
        icon: 'MagicStick',
        title: 'reader.themeToggle',
        options: () => THEMES.map((t, i) => ({
          key: String(i),
          label: t.label,
          color: t.bg,
          active: i === this.themeIdx,
        })),
        select: (key) => this.setTheme(Number(key)),
      },
    ]
  }

  async mount(container: HTMLElement): Promise<void> {
    this.container = container
    this.container.style.padding = '0'
    try {
      const buf = await this.opts.getSource()
      if (this.destroyed) return
      this.book = ePub(buf)
      // book.ready 解析后 spine/manifest/navigation 均就绪；rendition.display 的内部队列会等待 book.opened（资源替换）完成
      await this.book.ready
      if (this.destroyed || !this.book) return
      this.spineLength = this.book.packaging.spine.length
      this.state.total = this.spineLength
      this.buildToc()
      this.createStage(container)
      const rendition = this.createRendition()
      if (!rendition) return
      await rendition.display()
      if (this.destroyed) return
      this.state.loading = false
      this.state.ready = true
      void this.generateLocations()
    } catch (e: unknown) {
      this.state.loading = false
      this.state.error = e instanceof Error ? e.message : 'EPUB_LOAD_FAILED'
    }
  }

  // ---- 渲染 ----

  private createStage(container: HTMLElement): void {
    this.stageEl = document.createElement('div')
    this.stageEl.className = 'epub-stage'
    this.stageEl.style.width = '100%'
    this.stageEl.style.height = '100%'
    container.appendChild(this.stageEl)
  }

  private renditionOptions(): { width: string; height: string; flow: string; spread: string } {
    const paginated = this.turnMode === 'paginated'
    return {
      width: '100%',
      height: '100%',
      flow: paginated ? 'paginated' : 'scrolled-doc',
      spread: paginated && this.layout === 'double' ? 'auto' : 'none',
    }
  }

  private createRendition(): Rendition | null {
    if (!this.book || !this.stageEl) return null
    const rendition = this.book.renderTo(this.stageEl, this.renditionOptions())
    this.rendition = rendition
    this.applyThemes()
    rendition.on('relocated', this.onRelocated)
    return rendition
  }

  private scheduleRecreate(): void {
    this.recreateChain = this.recreateChain.then(() => this.recreateRendition())
  }

  private async recreateRendition(): Promise<void> {
    if (!this.book || !this.stageEl) return
    const cfi = this.currentCfi
    try {
      if (this.rendition) {
        this.rendition.off('relocated', this.onRelocated)
        this.rendition.destroy()
        this.rendition = null
      }
      this.stageEl.innerHTML = ''
      const rendition = this.createRendition()
      if (!rendition) return
      await rendition.display(cfi ?? undefined)
      if (this.destroyed) return
    } catch (e: unknown) {
      this.state.error = e instanceof Error ? e.message : 'EPUB_RENDER_FAILED'
    }
  }

  setPageTurnMode(mode: PageTurnMode): void {
    if (mode === this.turnMode) return
    this.turnMode = mode
    this.state.turnMode = mode
    // double+scroll 不被 epubjs 支持：切到滚动时自动降级为单页
    if (mode === 'scroll' && this.layout === 'double') {
      this.layout = 'single'
      this.state.layout = 'single'
    }
    this.scheduleRecreate()
  }

  setPageLayout(layout: PageLayout): void {
    // double+scroll 不被 epubjs 支持：滚动模式下忽略双页请求，保持单页
    if (layout === 'double' && this.turnMode === 'scroll') {
      this.state.layout = 'single'
      return
    }
    if (layout === this.layout) return
    this.layout = layout
    this.state.layout = layout
    this.scheduleRecreate()
  }

  // ---- 导航 ----

  private notifyPosition(jump: boolean): void {
    this.opts.onPositionChange?.(jump)
  }

  goPrev(): void {
    if (this.rendition) void this.rendition.prev()
    this.notifyPosition(false)
  }

  goNext(): void {
    if (this.rendition) void this.rendition.next()
    this.notifyPosition(false)
  }

  // 定位到 CFI 或百分比（如 "50%"、"0.5"、"50"），不发位置事件。
  private navigateToCoord(input: string): boolean {
    if (!this.rendition || !this.book) return false
    const s = input.trim()
    if (!s) return false
    if (s.toLowerCase().startsWith('epubcfi(')) {
      void this.rendition.display(s)
      return true
    }
    let pct: number
    if (s.endsWith('%')) {
      pct = parseFloat(s.slice(0, -1)) / 100
    } else {
      const n = parseFloat(s)
      if (isNaN(n)) return false
      pct = n <= 1 ? n : n / 100
    }
    if (isNaN(pct) || pct < 0 || pct > 1) return false
    if (!this.locationsReady) return false
    const target = this.book.locations.cfiFromPercentage(pct)
    void this.rendition.display(target)
    return true
  }

  goToLocation(input: string): boolean {
    const ok = this.navigateToCoord(input)
    if (ok) this.notifyPosition(true)
    return ok
  }

  // 当前位置的 CFI 即内容坐标。
  getContentCoord(): string | null {
    return this.currentCfi
  }

  goToContentCoord(coord: string): boolean {
    return this.navigateToCoord(coord)
  }

  goToToc(item: TocItem): void {
    if (!this.rendition) return
    const target = String(item.location)
    if (!target) return
    void this.rendition.display(target)
    this.notifyPosition(true)
  }

  // ---- 书签 ----

  // 书签展示名：优先百分比，退化为章节序号。
  private bookmarkLabel(): string {
    const loc = this.rendition?.location
    if (loc) {
      const pct =
        this.locationsReady && this.book && this.currentCfi
          ? this.book.locations.percentageFromCfi(this.currentCfi)
          : (typeof loc.start.percentage === 'number' ? loc.start.percentage : -1)
      if (typeof pct === 'number' && pct >= 0) return `${Math.round(pct * 100)}%`
      return `${loc.start.index + 1}`
    }
    return '-'
  }

  toggleBookmark(): void {
    toggleBookmarkSynced({
      state: this.state,
      opts: this.opts,
      refresh: () => this.updateBookmarkedFlag(),
      coord: this.getContentCoord(),
      label: this.bookmarkLabel(),
      matches: (b, coord) => String(b.location) === coord,
    })
  }

  removeBookmark(id: string): void {
    removeBookmarkSynced(
      { state: this.state, opts: this.opts, refresh: () => this.updateBookmarkedFlag() },
      id,
    )
  }

  goToBookmark(item: ReaderBookmark): void {
    if (this.navigateToCoord(String(item.location))) this.notifyPosition(true)
  }

  private updateBookmarkedFlag(): void {
    this.state.bookmarked =
      !!this.currentCfi && this.state.bookmarks.some((b) => b.location === this.currentCfi)
  }

  private onRelocated = (location: Location): void => {
    this.applyLocation(location)
  }

  private applyLocation(location: Location): void {
    const start = location.start
    this.currentCfi = start.cfi
    let pct = -1
    if (this.locationsReady && this.book && start.cfi) {
      pct = this.book.locations.percentageFromCfi(start.cfi)
    } else if (typeof start.percentage === 'number' && start.percentage > 0) {
      pct = start.percentage
    }
    if (pct >= 0) {
      this.state.progress = pct
      this.state.locationLabel = `${Math.round(pct * 100)}%`
    } else {
      const idx = start.index
      this.state.locationLabel = `${idx + 1} / ${this.spineLength}`
      this.state.progress = this.spineLength > 0 ? idx / this.spineLength : 0
    }
    this.state.canPrev = !location.atStart
    this.state.canNext = !location.atEnd
    this.updateBookmarkedFlag()
  }

  private async generateLocations(): Promise<void> {
    if (!this.book) return
    try {
      await this.book.locations.generate(1024)
      if (this.destroyed || !this.book) return
      this.locationsReady = true
      const loc = this.rendition?.location
      if (loc) this.applyLocation(loc)
    } catch {
      // 位置生成失败时进度退化为章节索引
    }
  }

  // ---- 目录 ----

  private buildToc(): void {
    if (!this.book) return
    const items: TocItem[] = []
    let counter = 0
    const walk = (navs: NavItem[], level: number): void => {
      for (const item of navs) {
        const n = counter++
        items.push({
          id: item.id || `toc-${n}`,
          label: (item.label || '').trim() || `Section ${n + 1}`,
          location: item.href,
          level,
        })
        const subs = item.subitems
        if (subs && subs.length) walk(subs, level + 1)
      }
    }
    walk(this.book.navigation.toc, 0)
    this.state.toc = items
  }

  // ---- 阅读设置 ----

  private loadSettings(): void {
    const ff = localStorage.getItem(LS_FONT_FAMILY)
    if (ff) this.fontFamily = ff
    const fs = parseInt(localStorage.getItem(LS_FONT_SIZE) ?? '', 10)
    if (!isNaN(fs)) this.fontSize = Math.min(36, Math.max(12, fs))
    const th = parseInt(localStorage.getItem(LS_THEME) ?? '', 10)
    if (!isNaN(th)) this.themeIdx = ((th % THEMES.length) + THEMES.length) % THEMES.length
    const ls = parseFloat(localStorage.getItem(LS_LINE_SPACING) ?? '')
    if (!isNaN(ls)) this.lineSpacing = Math.min(3, Math.max(1.2, ls))
    const rw = parseInt(localStorage.getItem(LS_READING_WIDTH) ?? '', 10)
    if (!isNaN(rw)) this.readingWidth = Math.min(1400, Math.max(480, rw))
  }

  private buildThemeStyles(): Record<string, Record<string, string>> {
    const theme = THEMES[this.themeIdx] ?? THEMES[0]!
    const font = `${this.fontFamily} !important`
    const size = `${this.fontSize}px !important`
    const lineH = `${this.lineSpacing} !important`
    const body: Record<string, string> = {
      'background-color': theme.bg,
      color: theme.text,
      'font-family': font,
      'font-size': size,
      'line-height': lineH,
    }
    // 仅滚动模式限制阅读宽度并居中；翻页模式由 epubjs 管理分栏布局
    if (this.turnMode === 'scroll') {
      body['max-width'] = `${this.readingWidth}px`
      body['margin'] = '0 auto'
      body['padding'] = '8px 16px'
    }
    return {
      html: { 'background-color': theme.bg, color: theme.text },
      body,
      p: { 'font-family': font, 'font-size': size, 'line-height': lineH },
    }
  }

  private applyThemes(): void {
    if (!this.rendition) return
    this.rendition.themes.register(THEME_NAME, this.buildThemeStyles())
    this.rendition.themes.select(THEME_NAME)
    if (this.container) {
      const theme = THEMES[this.themeIdx] ?? THEMES[0]!
      this.container.style.background = theme.bg
    }
  }

  private setFontSize(v: number): void {
    this.fontSize = Math.min(36, Math.max(12, Math.round(v)))
    localStorage.setItem(LS_FONT_SIZE, String(this.fontSize))
    this.applyThemes()
  }

  private setTheme(idx: number): void {
    this.themeIdx = ((idx % THEMES.length) + THEMES.length) % THEMES.length
    localStorage.setItem(LS_THEME, String(this.themeIdx))
    this.applyThemes()
  }

  destroy(): void {
    this.destroyed = true
    if (this.rendition) {
      this.rendition.off('relocated', this.onRelocated)
      this.rendition.destroy()
      this.rendition = null
    }
    if (this.book) {
      this.book.destroy()
      this.book = null
    }
    if (this.stageEl?.parentNode) {
      this.stageEl.parentNode.removeChild(this.stageEl)
      this.stageEl = null
    }
    if (this.container) {
      this.container.style.background = ''
    }
  }
}
