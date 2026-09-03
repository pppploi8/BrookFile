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

const VIEWPORT_PAD = 12 // 与 .reader-viewport 的 padding 保持一致
const SPREAD_GAP = 16 // 双页对开时两页之间的间距
const CHAPTER_GAP = 32 // 滚动模式下相邻块之间的间距
const PAGE_PAD_X = 36 // 翻页模式页内左右边距
const PAGE_PAD_Y = 20 // 翻页模式页内上下边距
const SLAB_KEEP = 8 // 双页滚动时视口外保留的已渲染对开屏数量

// TXT 阅读器设置均以该前缀存入 localStorage。
const LS_PREFIX = 'brookfile.reader.txt.'
const LS_FONT_FAMILY = LS_PREFIX + 'fontFamily'
const LS_FONT_SIZE = LS_PREFIX + 'fontSize'
const LS_THEME = LS_PREFIX + 'theme'
const LS_LINE_SPACING = LS_PREFIX + 'lineSpacing'
const LS_READING_WIDTH = LS_PREFIX + 'readingWidth'
const LS_READING_AUTO = LS_PREFIX + 'readingAuto'

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

// 后端提供的章节索引项（与 EngineOptions.chapters 同构）。
interface TxtChapter {
  chapter_no: number
  title: string | null
  location: string
}

interface PageMetrics {
  pageW: number
  pageH: number
  clipW: number
  clipH: number
  pitch: number
}

interface PageSlot {
  box: HTMLElement
  clip: HTMLElement
}

interface PagePos {
  ch: number
  col: number
}

// TXT 渲染引擎：
// 滚动+单页：以「章」为最小渲染单元，仅渲染当前章 ± 缓冲章（IntersectionObserver 懒加载）。
// 翻页模式：每章按视口尺寸切成固定大小的页（CSS 多列），页序跨章连续，翻页即翻页而非翻章；
//   双页对开显示左右两张连续页，章节边界处右页即下一章首页。
// 滚动+双页：与翻页双页相同的连续页序，按「左页+右页」组成对开屏，对开屏纵向堆叠滚动，
//   屏内容沿页链按需精确推进（顺序滚动均摊 O(1)），屏数用估算法生成滚动结构。
// 文本以 UTF-8 解码，章节 location 为字符偏移；进度按当前字符偏移计算，内容坐标形如 "chapterNo:charOffset"。
export class TxtEngine implements ReaderEngine {
  readonly format = 'txt' as const
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
  private text = ''
  private chapters: TxtChapter[] = []
  // 各章起始字符偏移（与 chapters 一一对应，首章强制为 0）。
  private chapterStarts: number[] = []
  // 各章正文起点（跳过与标题重复的原始标题行）；章节边界仍以 chapterStarts 为准。
  private bodyStarts: number[] = []

  private scrollEl: HTMLElement | null = null
  private contentEl: HTMLElement | null = null
  private observer: IntersectionObserver | null = null

  private turnMode: PageTurnMode = 'scroll'
  private layout: PageLayout = 'single'

  // 滚动+单页：每个 spread 是一章索引。
  private spreads: number[][] = []
  private spreadEls: HTMLElement[] = []
  private currentSpread = 0
  private currentChapter = 0
  // 当前阅读位置的字符偏移，用于进度与内容坐标。
  private currentOffset = 0

  private renderedSpreads = new Set<number>()

  private scrollRaf = 0
  private destroyed = false

  // 滚动+单页的字符级锚定循环（见 startScrollAnchor）。
  private anchorReq = 0
  private anchorState: { idx: number; align: 'char' | 'top'; rel: number; stable: number; deadline: number } | null = null

  // 分页状态（翻页模式 / 滚动+双页共用）。
  private pm: PageMetrics | null = null
  private vpW = 0
  private vpH = 0
  private pageAreaEl: HTMLElement | null = null
  private pageAreaSig = ''
  private pageSlots: PageSlot[] = []
  private measurerEl: HTMLElement | null = null
  private measurerClip: HTMLElement | null = null
  private pageCountCache = new Map<number, number>()
  private curCol = 0
  // 当前页首字符的绝对偏移，用于改字号/改版面/改视口后重新定位。
  private anchorOffset = 0
  private resizeObserver: ResizeObserver | null = null
  private resizeTimer = 0
  // 滚动+双页：已精确推进的对开屏页位（l 为左页，r 为右页；书末 r 与 l 相同表示空白页）。
  private slabPages: { l: PagePos; r: PagePos }[] = []

  // 阅读设置（构造时从 localStorage 载入）。
  private fontFamily = FONT_FAMILIES[0]!
  private fontSize = 18
  private themeIdx = 0
  private lineSpacing = 1.8
  private readingWidth = 820
  // 自适应宽度：内容区始终铺满视口可用宽度（同 PDF 的自适应宽度按钮）。
  private readingAuto = false

  // 当前生效的内容区宽度。
  private effectiveWidth(): number {
    if (!this.readingAuto) return this.readingWidth
    return Math.max(480, (this.scrollEl?.clientWidth ?? this.readingWidth) - VIEWPORT_PAD * 2)
  }

  constructor(opts: EngineOptions) {
    this.opts = opts
    this.loadSettings()
    // 窄屏下内容区宽度被视口封顶，宽度调节与适应宽度没有效果，不提供这组按钮。
    const narrow = window.innerWidth <= 768
    this.actions = [
      ...(narrow
        ? []
        : [
            { key: 'txt-width-dec', icon: 'Minus', title: 'reader.widthSmaller', run: () => this.setReadingWidth(this.effectiveWidth() - 80) },
            { key: 'txt-width-inc', icon: 'Plus', title: 'reader.widthLarger', run: () => this.setReadingWidth(this.effectiveWidth() + 80) },
            { key: 'txt-fit', icon: 'Aim', title: 'reader.fitWidth', run: () => this.setFitWidth() },
          ]),
      { key: 'txt-font', icon: 'Operation', title: 'reader.fontSettings', options: () => FONT_SIZES.map((s) => ({ key: String(s), text: `${s} px`, active: s === this.fontSize })), select: (key) => this.setFontSize(Number(key)) },
      {
        key: 'txt-theme',
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
    this.scrollEl = container
    container.addEventListener('scroll', this.onScroll, { passive: true })
    container.addEventListener('wheel', this.cancelAnchor, { passive: true })
    container.addEventListener('touchmove', this.cancelAnchor, { passive: true })
    container.addEventListener('keydown', this.cancelAnchor)
    container.addEventListener('pointerdown', this.cancelAnchor, { passive: true })
    this.contentEl = document.createElement('div')
    this.contentEl.className = 'txt-content'
    container.appendChild(this.contentEl)
    this.applyStyles()
    this.resizeObserver = new ResizeObserver(this.onViewportResize)
    this.resizeObserver.observe(container)

    try {
      const buf = await this.opts.getSource()
      if (this.destroyed) return
      this.text = new TextDecoder().decode(buf)
      this.normalizeChapters(this.opts.chapters)
      this.state.total = this.chapters.length
      this.buildToc()
      this.rebuildScrollDom()
      this.applyMode()

      this.state.loading = false
      this.state.ready = true
      this.updateCurrentSpread()
      this.updateNav()
      this.setPlaceholders()
      this.renderSpread(this.currentSpread, true)
    } catch (e: unknown) {
      this.state.loading = false
      this.state.error = e instanceof Error ? e.message : 'TXT_LOAD_FAILED'
    }
  }

  // ---- 章节归一化 ----

  // 将后端章节按 chapter_no 排序，解析 location 为字符偏移并保证单调递增、
  // 首章从 0 开始（避免丢失正文前的内容）。无章节时退化为单章覆盖全文。
  private normalizeChapters(input: TxtChapter[] | undefined): void {
    const list = input && input.length > 0 ? input.slice() : [{ chapter_no: 1, title: null, location: '0' }]
    list.sort((a, b) => a.chapter_no - b.chapter_no)
    this.chapters = list
    this.chapterStarts = []
    this.bodyStarts = []
    let prev = 0
    for (let i = 0; i < list.length; i++) {
      let s = parseInt(list[i]!.location, 10)
      if (isNaN(s)) s = i === 0 ? 0 : prev
      if (i === 0) s = 0
      s = Math.max(0, Math.min(s, this.text.length))
      if (i > 0 && s <= prev) s = Math.min(prev + 1, this.text.length)
      this.chapterStarts.push(s)
      prev = s
      // 正文起点跳过与章节标题重复的原始标题行（标题已由样式节点渲染，避免出现两次）
      const label = this.chapters[i]?.title?.trim()
      let body = s
      if (label) {
        const p = this.text.indexOf(label, s)
        if (p >= 0 && p - s <= 120) {
          const lineStart = this.text.lastIndexOf('\n', p) + 1
          const nl = this.text.indexOf('\n', p)
          const lineEnd = nl >= 0 ? nl : this.text.length
          if (this.text.substring(lineStart, lineEnd).trim() === label) {
            body = nl >= 0 ? nl + 1 : this.text.length
          }
        }
      }
      this.bodyStarts.push(body)
    }
  }

  private chapterLength(ci: number): number {
    const start = this.bodyStarts[ci] ?? 0
    const end = ci + 1 < this.chapterStarts.length ? this.chapterStarts[ci + 1]! : this.text.length
    return Math.max(0, end - start)
  }

  private chapterText(ci: number): string {
    const start = this.bodyStarts[ci] ?? 0
    const end = ci + 1 < this.chapterStarts.length ? this.chapterStarts[ci + 1]! : this.text.length
    const a = Math.min(start, this.text.length)
    const b = Math.min(Math.max(end, a), this.text.length)
    return this.text.substring(a, b)
  }

  private chapterLabel(ci: number): string {
    const ch = this.chapters[ci]
    if (!ch) return ''
    const t = ch.title?.trim()
    return t ? t : `Chapter ${ch.chapter_no}`
  }

  private buildToc(): void {
    this.state.toc = this.chapters.map((ch, i) => ({
      id: `ch-${ch.chapter_no}`,
      label: this.chapterLabel(i),
      location: i,
      level: 0,
    }))
  }

  // ---- 版面 / 翻页模式 ----

  private isScrollDouble(): boolean {
    return this.turnMode === 'scroll' && this.layout === 'double'
  }

  // 重建滚动模式的 DOM 结构（单页=章堆叠；双页=对开屏堆叠）。
  private rebuildScrollDom(): void {
    if (this.isScrollDouble()) {
      this.buildSlabs()
      return
    }
    this.buildSpreads()
    this.buildDom()
  }

  private buildSpreads(): void {
    this.spreads = []
    const n = this.chapters.length
    if (n === 0) return
    if (this.layout === 'double') {
      for (let i = 0; i < n; i += 2) {
        const pair = [i]
        if (i + 1 < n) pair.push(i + 1)
        this.spreads.push(pair)
      }
    } else {
      for (let i = 0; i < n; i++) this.spreads.push([i])
    }
  }

  private buildDom(): void {
    if (!this.contentEl) return
    this.spreadEls = []
    this.renderedSpreads.clear()
    this.slabPages = []
    this.pageAreaEl = null
    this.pageSlots = []
    this.measurerEl = null
    this.measurerClip = null
    this.contentEl.innerHTML = ''
    for (let i = 0; i < this.spreads.length; i++) {
      const se = document.createElement('div')
      se.className = 'txt-spread' + (this.layout === 'double' ? ' double' : '')
      se.dataset.spread = String(i)
      // 双章明确横向排布（左右对开），避免 CSS 歧义导致上下堆叠。
      se.style.display = 'flex'
      se.style.flexDirection = 'row'
      se.style.flexWrap = 'nowrap'
      se.style.justifyContent = 'center'
      se.style.alignItems = 'flex-start'
      se.style.gap = `${SPREAD_GAP}px`
      se.style.width = '100%'
      se.style.alignSelf = 'stretch'
      this.contentEl.appendChild(se)
      this.spreadEls.push(se)
    }
    this.setPlaceholders()
  }

  // 双页对开时两页间隙正中的垂直分隔线，颜色跟随主题文字色。
  private buildDivider(): HTMLElement {
    const d = document.createElement('div')
    d.className = 'txt-page-divider'
    d.style.position = 'absolute'
    d.style.left = '50%'
    d.style.top = '0'
    d.style.bottom = '0'
    d.style.width = '1px'
    d.style.background = 'currentColor'
    d.style.opacity = '0.25'
    d.style.pointerEvents = 'none'
    return d
  }

  // 滚动+双页：以估算法生成对开屏数量并搭建固定高度的屏结构（内容懒渲染）。
  private buildSlabs(): void {
    if (!this.contentEl) return
    this.computePageMetrics()
    this.spreads = []
    this.spreadEls = []
    this.renderedSpreads.clear()
    this.slabPages = []
    this.pageAreaEl = null
    this.pageSlots = []
    this.measurerEl = null
    this.measurerClip = null
    this.contentEl.innerHTML = ''
    const count = this.estimateSlabCount()
    for (let k = 0; k < count; k++) {
      const el = document.createElement('div')
      el.className = 'txt-slab'
      el.dataset.spread = String(k)
      el.style.position = 'relative'
      el.style.display = 'flex'
      el.style.flexDirection = 'row'
      el.style.flexWrap = 'nowrap'
      el.style.justifyContent = 'center'
      el.style.alignItems = 'stretch'
      el.style.gap = `${SPREAD_GAP}px`
      el.style.width = '100%'
      el.style.height = `${this.pm!.pageH}px`
      el.style.flexShrink = '0'
      el.appendChild(this.buildDivider())
      this.contentEl.appendChild(el)
      this.spreadEls.push(el)
    }
  }

  private estimateSlabCount(): number {
    let pages = 0
    for (let i = 0; i < this.chapters.length; i++) pages += this.estimatePageCount(i)
    return Math.max(1, Math.ceil(pages / 2) + 1)
  }

  // 实测第一章页数后按字数等比缩放估算各章页数（避免字符宽度经验值导致的累计偏差）。
  private estimatePageCount(ci: number): number {
    const pm = this.pm!
    const len = this.chapterLength(ci)
    const len0 = this.chapterLength(0)
    if (len0 > 0) {
      return Math.max(1, Math.round((this.pageCount(0) * len) / len0))
    }
    const charsPerLine = Math.max(8, Math.floor(pm.clipW / (this.fontSize * 0.6)))
    const lines = Math.ceil(len / charsPerLine)
    const estH = (lines + 2) * this.fontSize * this.lineSpacing
    return Math.max(1, Math.ceil(estH / pm.clipH))
  }

  // 沿页链精确推进到第 k 个对开屏（ advances 缓存章页数，顺序访问均摊 O(1)）。
  private ensureSlabsUpTo(k: number): void {
    if (this.chapters.length === 0) return
    while (this.slabPages.length <= k) {
      if (this.slabPages.length === 0) {
        const r = this.advancePage(0, 0, 1)
        this.slabPages.push({ l: { ch: 0, col: 0 }, r })
        continue
      }
      const prev = this.slabPages[this.slabPages.length - 1]!
      const l = this.advancePage(prev.r.ch, prev.r.col, 1)
      if (l.ch === prev.r.ch && l.col === prev.r.col) {
        this.slabPages.push({ l, r: { ch: l.ch, col: l.col } })
      } else {
        const r = this.advancePage(l.ch, l.col, 1)
        this.slabPages.push({ l, r })
      }
    }
  }

  private posCmp(a: PagePos, b: PagePos): number {
    if (a.ch !== b.ch) return a.ch - b.ch
    return a.col - b.col
  }

  // 定位包含 (ch, col) 这一页的对开屏索引（页面连续，首个 r >= 目标页的屏即覆盖目标）。
  private slabIndexForPos(ch: number, col: number): number {
    const target: PagePos = { ch, col }
    const lastCh = this.chapters.length - 1
    const lastCol = this.pageCount(lastCh) - 1
    let k = 0
    for (;;) {
      this.ensureSlabsUpTo(k)
      const p = this.slabPages[k]!
      const atEnd = this.posCmp(p.l, p.r) === 0 && this.posCmp(p.l, { ch: lastCh, col: lastCol }) === 0
      if (this.posCmp(p.r, target) >= 0) return k
      if (atEnd && this.posCmp(target, p.r) > 0) return k
      k++
    }
  }

  // 为尚未渲染的 spread 估计占位高度（按字符数推算），使懒加载观察器可触发、
  // 滚动高度尽量准确；已渲染的 spread 以其实测高度为准。
  private estimateChapterHeight(ci: number): number {
    const len = this.chapterLength(ci)
    const fs = this.fontSize
    const lineH = fs * this.lineSpacing
    const colW = this.layout === 'double' ? (this.effectiveWidth() - SPREAD_GAP) / 2 : this.effectiveWidth()
    const innerW = Math.max(120, colW - 48)
    // 混合 CJK/拉丁的平均字宽约为 0.6em。
    const charsPerLine = Math.max(8, Math.floor(innerW / (fs * 0.6)))
    const lines = Math.ceil(len / charsPerLine)
    return (lines + 2) * lineH + 48
  }

  private estimateSpreadHeight(spreadIdx: number): number {
    const spread = this.spreads[spreadIdx]
    if (!spread || spread.length === 0) return 200
    let h = 0
    for (const ci of spread) h = Math.max(h, this.estimateChapterHeight(ci))
    return h
  }

  private setPlaceholders(): void {
    if (this.isScrollDouble()) return
    for (let i = 0; i < this.spreadEls.length; i++) {
      const el = this.spreadEls[i]
      if (!el) continue
      if (el.querySelector('.txt-chapter')) {
        el.style.minHeight = ''
      } else {
        el.style.minHeight = `${this.estimateSpreadHeight(i)}px`
      }
    }
  }

  private applyMode(): void {
    if (!this.scrollEl || !this.contentEl) return
    if (this.turnMode === 'scroll') {
      this.scrollEl.style.overflow = 'auto'
      this.contentEl.style.display = 'flex'
      this.contentEl.style.flexDirection = 'column'
      this.contentEl.style.alignItems = 'center'
      this.contentEl.style.justifyContent = 'flex-start'
      this.contentEl.style.gap = `${CHAPTER_GAP}px`
      this.contentEl.style.width = '100%'
      this.contentEl.style.height = 'auto'
      this.spreadEls.forEach((el) => (el.style.display = 'flex'))
      if (this.pageAreaEl) this.pageAreaEl.style.display = 'none'
      this.enableObserver()
    } else {
      this.scrollEl.style.overflow = 'hidden'
      this.contentEl.style.display = 'flex'
      this.contentEl.style.flexDirection = 'column'
      this.contentEl.style.alignItems = 'center'
      this.contentEl.style.justifyContent = 'center'
      this.contentEl.style.gap = '0'
      this.contentEl.style.width = '100%'
      this.contentEl.style.height = 'auto'
      this.contentEl.style.minHeight = '100%'
      this.spreadEls.forEach((el) => (el.style.display = 'none'))
      if (this.pageAreaEl) this.pageAreaEl.style.display = 'flex'
      this.disableObserver()
      this.computePageMetrics()
      this.ensurePageArea()
      const ch = Math.min(this.currentChapter, this.chapters.length - 1)
      const rel = Math.max(0, this.currentOffset - (this.bodyStarts[ch] ?? 0))
      this.showPage(ch, this.columnForOffset(ch, rel))
    }
  }

  setPageTurnMode(mode: PageTurnMode): void {
    if (mode === this.turnMode) return
    this.turnMode = mode
    this.state.turnMode = mode
    if (mode === 'paginated') {
      this.applyMode()
      return
    }
    this.rebuildScrollDom()
    this.currentSpread = this.locateCurrentSpread()
    this.applyMode()
    this.renderSpread(this.currentSpread, true)
    if (this.scrollEl) {
      const el = this.spreadEls[this.currentSpread]
      if (el) this.scrollEl.scrollTo({ top: Math.max(0, el.offsetTop - VIEWPORT_PAD) })
    }
    this.updateCurrentSpread()
    this.updateNav()
  }

  setPageLayout(layout: PageLayout): void {
    if (layout === this.layout) return
    this.layout = layout
    this.state.layout = layout
    if (this.turnMode === 'paginated') {
      this.computePageMetrics()
      this.ensurePageArea()
      const rel = Math.max(0, this.anchorOffset - (this.bodyStarts[this.currentChapter] ?? 0))
      this.showPage(this.currentChapter, this.columnForOffset(this.currentChapter, rel))
      return
    }
    this.rebuildScrollDom()
    this.currentSpread = this.locateCurrentSpread()
    this.applyMode()
    this.renderSpread(this.currentSpread, true)
    if (this.scrollEl) {
      const el = this.spreadEls[this.currentSpread]
      if (el) this.scrollEl.scrollTo({ top: Math.max(0, el.offsetTop - VIEWPORT_PAD) })
    }
    this.updateCurrentSpread()
    this.updateNav()
  }

  // 由当前阅读位置（章节 + 锚点偏移）反推滚动单页/对开屏的索引。
  private locateCurrentSpread(): number {
    const ch = Math.min(this.currentChapter, this.chapters.length - 1)
    if (this.isScrollDouble()) {
      const rel = Math.max(0, this.anchorOffset - (this.bodyStarts[ch] ?? 0))
      return this.slabIndexForPos(ch, this.columnForOffset(ch, rel))
    }
    return this.spreadIndexForChapter(ch)
  }

  // ---- 滚动+单页渲染 ----

  // 章节块样式内联，避免 Vue scoped CSS 无法作用到动态创建的 DOM。
  // 字体/颜色/行高/字号均继承自 contentEl，故改设置后无需重渲染已显示的章。
  private createChapterBlock(ci: number): HTMLElement {
    const wrap = document.createElement('div')
    wrap.className = 'txt-chapter'
    wrap.dataset.chapter = String(ci)
    wrap.style.flex = '1 1 0'
    wrap.style.minWidth = '0'
    wrap.style.padding = '0 24px 48px'
    wrap.style.boxSizing = 'border-box'

    const titleEl = document.createElement('div')
    titleEl.className = 'txt-chapter-title'
    titleEl.textContent = this.chapterLabel(ci)
    titleEl.style.fontSize = '1.25em'
    titleEl.style.fontWeight = '600'
    titleEl.style.margin = '0 0 16px'
    titleEl.style.padding = '8px 0'
    titleEl.style.borderBottom = '1px solid currentColor'
    titleEl.style.opacity = '0.85'

    const bodyEl = document.createElement('div')
    bodyEl.className = 'txt-chapter-body'
    bodyEl.textContent = this.chapterText(ci)
    bodyEl.style.whiteSpace = 'pre-wrap'
    bodyEl.style.overflowWrap = 'break-word'

    wrap.appendChild(titleEl)
    wrap.appendChild(bodyEl)
    return wrap
  }

  private renderSpread(i: number, force = false): void {
    if (!force && this.renderedSpreads.has(i)) return
    const el = this.spreadEls[i]
    if (!el || this.destroyed) return
    if (this.isScrollDouble()) {
      this.renderSlab(i, el)
      return
    }
    const spread = this.spreads[i]
    if (!spread) return
    this.renderedSpreads.add(i)
    el.innerHTML = ''
    for (const ci of spread) {
      el.appendChild(this.createChapterBlock(ci))
    }
    this.setPlaceholders()
  }

  // 渲染第 k 个对开屏（页框惰性创建），并清理远离视口的屏以控制内存。
  private renderSlab(k: number, el: HTMLElement): void {
    this.ensureSlabsUpTo(k)
    const p = this.slabPages[k] ?? this.slabPages[this.slabPages.length - 1]
    if (!p) return
    el.innerHTML = ''
    el.appendChild(this.buildDivider())
    const clipL = this.buildPageBox(false).clip
    const clipR = this.buildPageBox(false).clip
    el.appendChild(clipL.parentNode!)
    el.appendChild(clipR.parentNode!)
    const innerL = this.renderChapterInto(clipL, p.l.ch, p.l.col)
    if (p.l.ch === p.r.ch && p.l.col === p.r.col) {
      clipR.innerHTML = ''
    } else {
      this.renderChapterInto(clipR, p.r.ch, p.r.col)
    }
    this.anchorOffset = (this.bodyStarts[p.l.ch] ?? 0) + this.offsetForColumn(innerL, p.l.ch, p.l.col)
    for (const idx of Array.from(this.renderedSpreads)) {
      if (Math.abs(idx - k) > SLAB_KEEP) {
        const far = this.spreadEls[idx]
        if (far) far.innerHTML = ''
        this.renderedSpreads.delete(idx)
      }
    }
    this.renderedSpreads.add(k)
  }

  private onIntersect = (entries: IntersectionObserverEntry[]): void => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        const i = Number((entry.target as HTMLElement).dataset.spread)
        this.renderSpread(i)
      }
    }
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

  // ---- 分页渲染（翻页模式 / 滚动+双页共用） ----

  private computePageMetrics(): void {
    if (!this.scrollEl) return
    this.vpW = this.scrollEl.clientWidth
    this.vpH = this.scrollEl.clientHeight
    const availW = Math.max(240, this.vpW - VIEWPORT_PAD * 2)
    const availH = Math.max(240, this.vpH - VIEWPORT_PAD * 2)
    const areaW = Math.min(this.effectiveWidth(), availW)
    const pageW = this.layout === 'double' ? Math.floor((areaW - SPREAD_GAP) / 2) : areaW
    const clipW = Math.max(120, pageW - PAGE_PAD_X * 2)
    this.pm = {
      pageW,
      pageH: availH,
      clipW,
      clipH: Math.max(120, availH - PAGE_PAD_Y * 2),
      pitch: clipW + SPREAD_GAP,
    }
    this.pageCountCache.clear()
  }

  private buildPageBox(absolute: boolean): PageSlot {
    const pm = this.pm!
    const box = document.createElement('div')
    box.className = 'txt-page'
    box.style.position = absolute ? 'absolute' : 'relative'
    box.style.width = `${pm.pageW}px`
    box.style.height = `${pm.pageH}px`
    box.style.overflow = 'hidden'
    box.style.flexShrink = '0'
    if (absolute) {
      box.style.left = '-100000px'
      box.style.top = '0'
      box.style.visibility = 'hidden'
      box.style.pointerEvents = 'none'
    }
    const clip = document.createElement('div')
    clip.className = 'txt-page-clip'
    clip.style.position = 'absolute'
    clip.style.left = `${PAGE_PAD_X}px`
    clip.style.right = `${PAGE_PAD_X}px`
    clip.style.top = `${PAGE_PAD_Y}px`
    clip.style.bottom = `${PAGE_PAD_Y}px`
    clip.style.overflow = 'hidden'
    box.appendChild(clip)
    return { box, clip }
  }

  private ensurePageArea(): void {
    if (!this.contentEl || !this.pm) return
    const sig = `${this.layout}|${this.pm.pageW}x${this.pm.pageH}`
    if (this.pageAreaEl && sig === this.pageAreaSig) return
    if (this.pageAreaEl) this.pageAreaEl.remove()
    const area = document.createElement('div')
    area.className = 'txt-page-area'
    area.style.position = 'relative'
    area.style.display = 'flex'
    area.style.flexDirection = 'row'
    area.style.flexWrap = 'nowrap'
    area.style.justifyContent = 'center'
    area.style.alignItems = 'stretch'
    area.style.gap = `${SPREAD_GAP}px`
    if (this.layout === 'double') {
      area.appendChild(this.buildDivider())
    }
    const count = this.layout === 'double' ? 2 : 1
    this.pageSlots = []
    for (let i = 0; i < count; i++) {
      const slot = this.buildPageBox(false)
      area.appendChild(slot.box)
      this.pageSlots.push(slot)
    }
    this.contentEl.appendChild(area)
    this.pageAreaEl = area
    this.pageAreaSig = sig
  }

  private ensureMeasurer(): void {
    if (!this.contentEl) return
    if (this.measurerEl) return
    const slot = this.buildPageBox(true)
    this.measurerEl = slot.box
    this.measurerClip = slot.clip
    this.contentEl.appendChild(slot.box)
  }

  // 将一章渲染进给定页框，平移到第 col 列（页），返回内层多列元素供测量。
  private renderChapterInto(clip: HTMLElement, ch: number, col: number): HTMLElement {
    const pm = this.pm!
    clip.innerHTML = ''
    const inner = document.createElement('div')
    inner.className = 'txt-page-inner'
    inner.style.width = `${pm.clipW}px`
    inner.style.height = `${pm.clipH}px`
    inner.style.columnWidth = `${pm.clipW}px`
    inner.style.columnGap = `${SPREAD_GAP}px`
    inner.style.columnFill = 'auto'
    inner.style.transform = `translateX(${-col * pm.pitch}px)`

    const titleEl = document.createElement('div')
    titleEl.className = 'txt-page-title'
    titleEl.textContent = this.chapterLabel(ch)
    titleEl.style.fontSize = '1.25em'
    titleEl.style.fontWeight = '600'
    titleEl.style.margin = '0 0 16px'
    titleEl.style.padding = '8px 0'
    titleEl.style.borderBottom = '1px solid currentColor'
    titleEl.style.opacity = '0.85'

    const bodyEl = document.createElement('div')
    bodyEl.className = 'txt-page-body'
    bodyEl.textContent = this.chapterText(ch)
    bodyEl.style.whiteSpace = 'pre-wrap'
    bodyEl.style.overflowWrap = 'break-word'

    inner.appendChild(titleEl)
    inner.appendChild(bodyEl)
    clip.appendChild(inner)
    return inner
  }

  private pageCount(ch: number): number {
    const cached = this.pageCountCache.get(ch)
    if (cached !== undefined) return cached
    if (!this.pm || this.chapters.length === 0) return 1
    this.ensureMeasurer()
    const inner = this.renderChapterInto(this.measurerClip!, ch, 0)
    const cols = Math.max(1, Math.round((inner.scrollWidth + SPREAD_GAP) / this.pm.pitch))
    this.pageCountCache.set(ch, cols)
    return cols
  }

  // 在连续页序中前进/后退 step 页，跨章时进入下一章首页/上一章末页，两端夹紧。
  private advancePage(ch: number, col: number, step: number): { ch: number; col: number } {
    const n = this.chapters.length
    if (n === 0) return { ch: 0, col: 0 }
    let c = Math.min(Math.max(0, ch), n - 1)
    let x = Math.min(Math.max(0, col), this.pageCount(c) - 1)
    if (step > 0) {
      let k = step
      while (k > 0) {
        const pc = this.pageCount(c)
        if (x + k <= pc - 1) {
          x += k
          k = 0
        } else {
          k -= pc - x
          if (c + 1 >= n) {
            x = this.pageCount(c) - 1
            k = 0
          } else {
            c += 1
            x = 0
          }
        }
      }
    } else if (step < 0) {
      let k = -step
      while (k > 0) {
        if (x >= k) {
          x -= k
          k = 0
        } else {
          k -= x + 1
          if (c <= 0) {
            x = 0
            k = 0
          } else {
            c -= 1
            x = this.pageCount(c) - 1
          }
        }
      }
    }
    return { ch: c, col: x }
  }

  // 计算某字符在当前页栅格中落在第几列（页）。
  private columnOfChar(inner: HTMLElement, ch: number, off: number): number {
    const pm = this.pm
    if (!pm) return 0
    const body = inner.querySelector('.txt-page-body')
    const node = body ? body.firstChild : null
    if (!node) return 0
    const len = this.chapterLength(ch)
    const o = Math.min(Math.max(0, off), len)
    const r = document.createRange()
    r.setStart(node, o)
    r.setEnd(node, Math.min(o + 1, len))
    const rects = r.getClientRects()
    const rect = rects.length > 0 ? rects[rects.length - 1]! : r.getBoundingClientRect()
    const innerLeft = inner.getBoundingClientRect().left
    return Math.max(0, Math.floor((rect.left - innerLeft) / pm.pitch + 0.0001))
  }

  // 二分求第 col 页的首字符偏移（章内相对偏移）。
  private offsetForColumn(inner: HTMLElement, ch: number, col: number): number {
    const len = this.chapterLength(ch)
    if (col <= 0 || len === 0) return 0
    let lo = 0
    let hi = len
    while (lo < hi) {
      const mid = (lo + hi) >> 1
      if (this.columnOfChar(inner, ch, mid) >= col) hi = mid
      else lo = mid + 1
    }
    return Math.min(lo, len)
  }

  private columnForOffset(ch: number, relOffset: number): number {
    if (!this.pm) return 0
    const rel = Math.min(Math.max(0, relOffset), this.chapterLength(ch))
    if (rel <= 0) return 0
    this.ensureMeasurer()
    const inner = this.renderChapterInto(this.measurerClip!, ch, 0)
    return Math.min(this.columnOfChar(inner, ch, rel), this.pageCount(ch) - 1)
  }

  private showPage(ch: number, col: number): void {
    if (!this.pm || this.chapters.length === 0) return
    const c = Math.min(Math.max(0, ch), this.chapters.length - 1)
    const x = Math.min(Math.max(0, col), this.pageCount(c) - 1)
    this.ensurePageArea()
    const leftSlot = this.pageSlots[0]
    if (!leftSlot) return
    const innerL = this.renderChapterInto(leftSlot.clip, c, x)
    if (this.layout === 'double' && this.pageSlots[1]) {
      const rightSlot = this.pageSlots[1]!
      const nxt = this.advancePage(c, x, 1)
      if (nxt.ch === c && nxt.col === x) {
        rightSlot.clip.innerHTML = ''
      } else {
        this.renderChapterInto(rightSlot.clip, nxt.ch, nxt.col)
      }
    }
    this.currentChapter = c
    this.curCol = x
    this.currentSpread = this.spreadIndexForChapter(c)
    this.anchorOffset = (this.bodyStarts[c] ?? 0) + this.offsetForColumn(innerL, c, x)

    const start = this.bodyStarts[c] ?? 0
    const len = this.chapterLength(c)
    const pc = this.pageCount(c)
    this.currentOffset = Math.min(this.text.length, start + Math.floor((len * x) / pc))
    this.updateLocationLabel(c)
    this.state.progress = this.text.length > 0 ? this.currentOffset / this.text.length : 0
    this.state.canPrev = !(c === 0 && x === 0)
    this.state.canNext = !(c === this.chapters.length - 1 && x === pc - 1)
  }

  private repaginateAtAnchor(): void {
    this.computePageMetrics()
    this.ensurePageArea()
    const rel = Math.max(0, this.anchorOffset - (this.bodyStarts[this.currentChapter] ?? 0))
    this.showPage(this.currentChapter, this.columnForOffset(this.currentChapter, rel))
  }

  // 滚动+双页：按锚点重建对开屏结构并回到原位置。
  private rebuildSlabsAtAnchor(): void {
    this.rebuildScrollDom()
    this.currentSpread = this.locateCurrentSpread()
    this.applyMode()
    this.renderSpread(this.currentSpread, true)
    if (this.scrollEl) {
      const el = this.spreadEls[this.currentSpread]
      if (el) this.scrollEl.scrollTo({ top: Math.max(0, el.offsetTop - VIEWPORT_PAD) })
    }
    this.updateCurrentSpread()
    this.updateNav()
  }

  private onViewportResize = (): void => {
    if (this.resizeTimer) window.clearTimeout(this.resizeTimer)
    this.resizeTimer = window.setTimeout(() => {
      this.resizeTimer = 0
      if (this.destroyed || !this.scrollEl) return
      if (this.scrollEl.clientWidth === this.vpW && this.scrollEl.clientHeight === this.vpH) return
      if (this.turnMode === 'paginated') {
        this.repaginateAtAnchor()
      } else if (this.isScrollDouble()) {
        this.rebuildSlabsAtAnchor()
      } else if (this.readingAuto) {
        this.applyStyles()
      }
    }, 200)
  }

  // ---- 导航 ----

  private spreadIndexForChapter(ci: number): number {
    for (let i = 0; i < this.spreads.length; i++) {
      if (this.spreads[i]?.includes(ci)) return i
    }
    return 0
  }

  private chapterIndexByNo(no: number): number {
    for (let i = 0; i < this.chapters.length; i++) {
      if (this.chapters[i]?.chapter_no === no) return i
    }
    return -1
  }

  private scrollToSpread(i: number, smooth: boolean): void {
    this.cancelAnchor()
    if (this.spreadEls.length === 0) return
    const idx = Math.min(Math.max(0, i), this.spreadEls.length - 1)
    this.currentSpread = idx
    const el = this.spreadEls[idx]
    if (el && this.scrollEl) {
      const top = Math.max(0, el.offsetTop - VIEWPORT_PAD)
      const near = Math.abs(top - this.scrollEl.scrollTop) < this.scrollEl.clientHeight * 3
      this.scrollEl.scrollTo({ top, behavior: smooth && near ? 'smooth' : 'auto' })
    }
    this.updateCurrentSpread()
    this.updateNav()
  }

  // ---- 位置上报与内容坐标 ----

  private notifyPosition(jump: boolean): void {
    this.opts.onPositionChange?.(jump)
  }

  // ---- 滚动+单页的字符级定位 ----

  // 章节渲染后 .txt-chapter-body 是单一文本节点，用 Range 直接取字符偏移的精确像素位置
  // （滚动内容坐标系；未渲染返回 null）。
  private charY(el: HTMLElement, rel: number): number | null {
    const node = el.querySelector('.txt-chapter-body')?.firstChild
    if (!this.scrollEl || !node || node.nodeType !== Node.TEXT_NODE) return null
    const text = node as Text
    if (text.length === 0) return null
    const off = Math.min(text.length - 1, Math.max(0, Math.round(rel)))
    const range = document.createRange()
    range.setStart(text, off)
    range.setEnd(text, off + 1)
    const rect = range.getBoundingClientRect()
    if (rect.height === 0 && rect.width === 0) return null
    return rect.top + rect.height / 2 - this.scrollEl.getBoundingClientRect().top + this.scrollEl.scrollTop
  }

  // 二分查找已渲染章内位于滚动内容坐标 y 处的字符偏移（未渲染返回 null）。
  private charAtY(el: HTMLElement, y: number): number | null {
    const node = el.querySelector('.txt-chapter-body')?.firstChild
    if (!this.scrollEl || !node || node.nodeType !== Node.TEXT_NODE) return null
    const text = node as Text
    if (text.length === 0) return null
    const base = this.scrollEl.getBoundingClientRect().top - this.scrollEl.scrollTop
    const range = document.createRange()
    const yOf = (off: number): number => {
      const o = Math.min(off, text.length - 1)
      range.setStart(text, o)
      range.setEnd(text, o + 1)
      const rect = range.getBoundingClientRect()
      if (rect.height === 0 && rect.width === 0) return Number.NaN
      return rect.top + rect.height / 2 - base
    }
    let lo = 0
    let hi = text.length - 1
    let best = -1
    while (lo <= hi) {
      const mid = (lo + hi) >> 1
      const yy = yOf(mid)
      if (Number.isNaN(yy)) return null
      if (yy <= y) {
        best = mid
        lo = mid + 1
      } else {
        hi = mid - 1
      }
    }
    return Math.max(0, best)
  }

  // 启动锚定循环：恢复/跳转时目标章可能尚未渲染（占位高度为估算），相邻章节渲染也会
  // 移动目标位置；循环先渲染目标章，再把目标位置（渲染后按字符偏移精确取位）持续对齐
  // 到视口，直到布局稳定、超时或用户接管输入。首次定位同步执行（不等一帧），
  // 后续精化走 requestAnimationFrame。
  private startScrollAnchor(idx: number, align: 'char' | 'top', rel = 0): void {
    if (!this.scrollEl || idx < 0 || idx >= this.spreadEls.length) return
    this.anchorState = { idx, align, rel, stable: 0, deadline: Date.now() + 5000 }
    this.anchorStep()
  }

  private cancelAnchor = (): void => {
    this.anchorState = null
  }

  private anchorStep = (): void => {
    this.anchorReq = 0
    const a = this.anchorState
    if (!a || this.destroyed || !this.scrollEl || this.turnMode !== 'scroll' || this.isScrollDouble()) {
      this.anchorState = null
      return
    }
    const el = this.spreadEls[a.idx]
    if (!el) {
      this.anchorState = null
      return
    }
    this.renderSpread(a.idx)
    let desired: number
    if (a.align === 'top') {
      desired = el.offsetTop - VIEWPORT_PAD
    } else {
      const ci = this.spreads[a.idx]?.[0] ?? 0
      const len = Math.max(1, this.chapterLength(ci))
      const rel = Math.min(a.rel, len)
      const y = this.charY(el, rel) ?? el.offsetTop + (rel / len) * el.offsetHeight
      desired = y - this.scrollEl.clientHeight / 2
    }
    const top = Math.max(0, desired)
    if (Math.abs(this.scrollEl.scrollTop - top) > 1) {
      this.scrollEl.scrollTop = top
      a.stable = 0
    } else {
      a.stable++
    }
    this.computeCurrentFromScroll()
    if (a.stable >= 30 || Date.now() > a.deadline) {
      this.anchorState = null
      return
    }
    this.anchorReq = requestAnimationFrame(this.anchorStep)
  }

  // 当前位置的内容坐标："章节号:章内相对字符偏移"（字体无关）。
  getContentCoord(): string | null {
    if (this.chapters.length === 0) return null
    const ci = Math.min(this.currentChapter, this.chapters.length - 1)
    const no = this.chapters[ci]?.chapter_no ?? ci + 1
    const rel = Math.max(0, this.currentOffset - (this.bodyStarts[ci] ?? 0))
    return `${no}:${rel}`
  }

  // 解析 "chapterNo:charOffset" 或单独章节号 → { 章节下标, 章内相对偏移 }。
  private parseCoord(coord: string): { ci: number; rel: number } | null {
    const sep = coord.indexOf(':')
    const no = parseInt(sep >= 0 ? coord.slice(0, sep) : coord, 10)
    if (isNaN(no)) return null
    const ci = this.chapterIndexByNo(no)
    if (ci < 0) return null
    const rel = sep >= 0 ? parseInt(coord.slice(sep + 1), 10) : 0
    return { ci, rel: isNaN(rel) ? 0 : Math.max(0, rel) }
  }

  goPrev(): void {
    if (this.turnMode === 'paginated') {
      const step = this.layout === 'double' ? 2 : 1
      const t = this.advancePage(this.currentChapter, this.curCol, -step)
      if (t.ch !== this.currentChapter || t.col !== this.curCol) this.showPage(t.ch, t.col)
      this.notifyPosition(false)
      return
    }
    this.scrollToSpread(this.currentSpread - 1, true)
    this.notifyPosition(false)
  }

  goNext(): void {
    if (this.turnMode === 'paginated') {
      const step = this.layout === 'double' ? 2 : 1
      const t = this.advancePage(this.currentChapter, this.curCol, step)
      if (t.ch !== this.currentChapter || t.col !== this.curCol) this.showPage(t.ch, t.col)
      this.notifyPosition(false)
      return
    }
    this.scrollToSpread(this.currentSpread + 1, true)
    this.notifyPosition(false)
  }

  // 解析用户输入并定位（不发位置事件，由公开导航方法决定上报语义）。
  private navigateToCoord(input: string, smooth: boolean): boolean {
    const s = input.trim()
    if (!s) return false
    const sep = s.indexOf(':')
    const head = sep >= 0 ? s.slice(0, sep) : s
    const off = sep >= 0 ? parseInt(s.slice(sep + 1), 10) : NaN
    const no = parseInt(head, 10)
    if (isNaN(no)) return false
    const ci = this.chapterIndexByNo(no)
    if (ci < 0) return false
    if (this.turnMode === 'paginated') {
      this.showPage(ci, this.columnForOffset(ci, isNaN(off) ? 0 : off))
      return true
    }
    if (this.isScrollDouble()) {
      const col = this.columnForOffset(ci, isNaN(off) ? 0 : off)
      this.scrollToSpread(this.slabIndexForPos(ci, col), smooth)
      return true
    }
    // 滚动+单页：渲染目标章并锚定到字符偏移的精确像素位置（布局稳定前持续对齐）。
    const idx = this.spreadIndexForChapter(ci)
    if (!isNaN(off) && off > 0) {
      this.startScrollAnchor(idx, 'char', off)
    } else {
      this.startScrollAnchor(idx, 'top')
    }
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

  goToToc(item: TocItem): void {
    const ci = Number(item.location)
    if (isNaN(ci) || ci < 0 || ci >= this.chapters.length) return
    if (this.turnMode === 'paginated') {
      this.showPage(ci, 0)
      this.notifyPosition(true)
      return
    }
    if (this.isScrollDouble()) {
      this.scrollToSpread(this.slabIndexForPos(ci, 0), true)
      this.notifyPosition(true)
      return
    }
    this.startScrollAnchor(this.spreadIndexForChapter(ci), 'top')
    this.notifyPosition(true)
  }

  // ---- 书签 ----

  // 当前位置的页定位令牌：章节 + 页列（滚动单页模式无页概念，列恒为 0）。
  private bookmarkToken(): { ch: number; col: number } {
    const ch = Math.min(this.currentChapter, this.chapters.length - 1)
    const col = this.turnMode === 'scroll' && this.layout === 'single' ? 0 : this.curCol
    return { ch, col }
  }

  // 判断已有书签是否位于当前阅读位置：同章节内按页列（或滚动单页整章）比较。
  private sameBookmarkCoord(b: ReaderBookmark, coord: string): boolean {
    const a = this.parseCoord(String(b.location))
    const c = this.parseCoord(coord)
    if (!a || !c) return !!a && !!c && String(b.location) === coord
    if (a.ci !== c.ci) return false
    if (this.turnMode === 'scroll' && this.layout === 'single') return true
    return this.columnForOffset(a.ci, a.rel) === this.columnForOffset(c.ci, c.rel)
  }

  toggleBookmark(): void {
    if (this.chapters.length === 0) return
    toggleBookmarkSynced({
      state: this.state,
      opts: this.opts,
      refresh: () => this.updateBookmarkedFlag(),
      coord: this.getContentCoord(),
      label: this.chapterLabel(Math.min(this.currentChapter, this.chapters.length - 1)),
      matches: (b, coord) => this.sameBookmarkCoord(b, coord),
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
    if (this.state.bookmarks.length === 0) {
      this.state.bookmarked = false
      return
    }
    const token = this.bookmarkToken()
    this.state.bookmarked = this.state.bookmarks.some((b) => {
      const parsed = this.parseCoord(String(b.location))
      if (!parsed || parsed.ci !== token.ch) return false
      if (this.turnMode === 'scroll' && this.layout === 'single') return true
      return this.columnForOffset(parsed.ci, parsed.rel) === token.col
    })
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
    const mid = this.scrollEl.scrollTop + this.scrollEl.clientHeight / 2
    let cur = 0
    for (let i = 0; i < this.spreadEls.length; i++) {
      const el = this.spreadEls[i]
      if (!el) break
      if (el.offsetTop <= mid) cur = i
      else break
    }
    const el = this.spreadEls[cur]
    const changed = cur !== this.currentSpread
    // 已渲染的章用字符级精确反查视口中线位置（未渲染时退化为比例估算）。
    const exact = el && !this.isScrollDouble() ? this.charAtY(el, mid) : null
    if (exact !== null) {
      this.setCurrent(cur, 0, exact)
    } else {
      let frac = 0
      if (el && el.offsetHeight > 0) {
        frac = Math.min(1, Math.max(0, (mid - el.offsetTop) / el.offsetHeight))
      }
      this.setCurrent(cur, frac)
    }
    if (changed) this.updateNav()
    this.notifyPosition(false)
  }

  // 由当前 spread + 章内滚动比例计算当前章节与字符偏移，并更新进度/标签；
  // exactRel 为已渲染章的精确字符偏移（优先于比例换算）。
  private setCurrent(spreadIdx: number, frac = 0, exactRel?: number): void {
    if (this.isScrollDouble()) {
      this.setCurrentSlab(spreadIdx, frac)
      return
    }
    this.currentSpread = spreadIdx
    const spread = this.spreads[spreadIdx]
    if (!spread) return
    const ci = spread[0]!
    this.currentChapter = ci
    const start = this.bodyStarts[ci] ?? 0
    const len = this.chapterLength(ci)
    const rel = exactRel !== undefined
      ? Math.min(len, Math.max(0, exactRel))
      : Math.min(len, Math.max(0, frac * len))
    this.currentOffset = Math.min(this.text.length, Math.round(start + rel))
    this.updateLocationLabel(ci)
    this.state.progress = this.text.length > 0 ? this.currentOffset / this.text.length : 0
  }

  private setCurrentSlab(k: number, frac: number): void {
    if (this.chapters.length === 0) return
    this.ensureSlabsUpTo(k)
    const p = this.slabPages[Math.min(k, this.slabPages.length - 1)]!
    this.currentSpread = k
    this.currentChapter = p.l.ch
    this.curCol = p.l.col
    const start = this.bodyStarts[p.l.ch] ?? 0
    const len = this.chapterLength(p.l.ch)
    const pc = this.pageCount(p.l.ch)
    this.currentOffset = Math.min(this.text.length, start + Math.floor((len * (p.l.col + frac)) / pc))
    this.updateLocationLabel(p.l.ch)
    this.state.progress = this.text.length > 0 ? this.currentOffset / this.text.length : 0
  }

  private updateLocationLabel(ci: number): void {
    const no = this.chapters[ci]?.chapter_no ?? ci + 1
    if (this.chapters.length > 1) {
      this.state.locationLabel = `${no} / ${this.chapters.length}`
    } else {
      this.state.locationLabel = `${this.currentOffset} / ${this.text.length}`
    }
    this.updateBookmarkedFlag()
  }

  private updateCurrentSpread(): void {
    this.setCurrent(this.currentSpread, 0)
  }

  private updateNav(): void {
    const last = this.spreadEls.length - 1
    this.state.canPrev = this.currentSpread > 0
    this.state.canNext = this.currentSpread < last
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
    this.readingAuto = localStorage.getItem(LS_READING_AUTO) === '1'
  }

  private applyStyles(): void {
    if (!this.contentEl) return
    const theme = THEMES[this.themeIdx] ?? THEMES[0]!
    this.contentEl.style.fontFamily = this.fontFamily
    this.contentEl.style.color = theme.text
    this.contentEl.style.background = theme.bg
    this.contentEl.style.fontSize = `${this.fontSize}px`
    this.contentEl.style.lineHeight = String(this.lineSpacing)
    const availW = Math.max(240, (this.scrollEl?.clientWidth ?? 0) - VIEWPORT_PAD * 2)
    this.contentEl.style.maxWidth = `${Math.min(this.effectiveWidth(), availW)}px`
    this.contentEl.style.margin = '0 auto'
    if (this.scrollEl) this.scrollEl.style.background = theme.bg
  }

  private setFontSize(v: number): void {
    this.fontSize = Math.min(36, Math.max(12, Math.round(v)))
    localStorage.setItem(LS_FONT_SIZE, String(this.fontSize))
    this.applyStyles()
    this.refreshAfterSettingChange()
  }

  private setReadingWidth(v: number): void {
    this.readingAuto = false
    localStorage.setItem(LS_READING_AUTO, '0')
    this.readingWidth = Math.min(1400, Math.max(480, Math.round(v)))
    localStorage.setItem(LS_READING_WIDTH, String(this.readingWidth))
    this.applyStyles()
    this.refreshAfterSettingChange()
  }

  // 自适应宽度：内容区铺满视口可用宽度，随窗口缩放联动。
  private setFitWidth(): void {
    this.readingAuto = true
    localStorage.setItem(LS_READING_AUTO, '1')
    this.applyStyles()
    this.refreshAfterSettingChange()
  }

  // 字号/内容区宽度等影响排版的设置变更后，按当前模式重排并保持阅读位置。
  private refreshAfterSettingChange(): void {
    if (this.turnMode === 'paginated') {
      this.repaginateAtAnchor()
      return
    }
    if (this.isScrollDouble()) {
      this.rebuildSlabsAtAnchor()
      return
    }
    this.setPlaceholders()
    requestAnimationFrame(() => {
      if (this.turnMode === 'scroll' && this.scrollEl) {
        const el = this.spreadEls[this.currentSpread]
        if (el) this.scrollEl.scrollTo({ top: Math.max(0, el.offsetTop - VIEWPORT_PAD) })
      }
    })
  }

  private setTheme(idx: number): void {
    this.themeIdx = ((idx % THEMES.length) + THEMES.length) % THEMES.length
    localStorage.setItem(LS_THEME, String(this.themeIdx))
    this.applyStyles()
  }

  destroy(): void {
    this.destroyed = true
    this.disableObserver()
    this.observer = null
    if (this.resizeObserver) {
      this.resizeObserver.disconnect()
      this.resizeObserver = null
    }
    if (this.resizeTimer) {
      window.clearTimeout(this.resizeTimer)
      this.resizeTimer = 0
    }
    if (this.scrollEl) {
      this.scrollEl.removeEventListener('scroll', this.onScroll)
      this.scrollEl.removeEventListener('wheel', this.cancelAnchor)
      this.scrollEl.removeEventListener('touchmove', this.cancelAnchor)
      this.scrollEl.removeEventListener('keydown', this.cancelAnchor)
      this.scrollEl.removeEventListener('pointerdown', this.cancelAnchor)
      this.scrollEl.style.overflow = ''
      this.scrollEl.style.background = ''
    }
    if (this.scrollRaf) cancelAnimationFrame(this.scrollRaf)
    if (this.anchorReq) cancelAnimationFrame(this.anchorReq)
    this.anchorState = null
    if (this.contentEl?.parentNode) {
      this.contentEl.parentNode.removeChild(this.contentEl)
      this.contentEl = null
    }
    this.pageAreaEl = null
    this.pageSlots = []
    this.measurerEl = null
    this.measurerClip = null
    this.slabPages = []
  }
}
