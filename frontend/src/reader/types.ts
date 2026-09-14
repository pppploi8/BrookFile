// 阅读器框架的公共类型定义。
// 三种格式（pdf / epub / txt）共用同一套 ReaderShell 外壳，
// 仅渲染引擎（ReaderEngine）不同，避免重复编写三套 UI。

export type BookFormat = 'txt' | 'epub' | 'pdf'

// 翻页模式：滚动（连续纵滚）/ 翻页（一屏一页、左右/上下翻）。
export type PageTurnMode = 'scroll' | 'paginated'

// 版面：单页（一屏一页）/ 双页（对开页，一屏两页）。
export type PageLayout = 'single' | 'double'

export interface TocItem {
  id: string
  label: string
  // 引擎自定义的定位令牌，由 goToToc 解释
  location: unknown
  level?: number
  children?: TocItem[]
}

// 书签：location 为引擎自定义的定位令牌，由 goToBookmark 解释。
export interface ReaderBookmark {
  id: string
  label: string
  location: unknown
}

// 引擎自定义的的工具栏操作（如 pdf 的缩放、txt 的字体大小等）。
// icon 为已在全局注册的 Element Plus 图标组件名（字符串），
// title 为 i18n 键。
// 提供 options 时渲染为下拉选择：options 返回候选项，选中后回调 select；
// label 为 i18n 键，text 为直接展示的纯文本（优先于 label）；否则渲染为普通按钮，点击执行 run。
export interface ReaderActionOption {
  key: string
  label?: string
  text?: string
  color?: string
  active?: boolean
}

export interface ReaderAction {
  key: string
  icon: string
  title: string
  run?: () => void
  options?: () => ReaderActionOption[]
  select?: (key: string) => void
}

export interface ReaderState {
  loading: boolean
  ready: boolean
  error: string | null
  // 0..1 的阅读进度
  progress: number
  // 当前位置的可读标签，如 "12 / 240" 或 "34%"
  locationLabel: string
  canPrev: boolean
  canNext: boolean
  toc: TocItem[]
  // 书签（内存态，随阅读器实例销毁）
  bookmarks: ReaderBookmark[]
  // 当前位置是否已加书签
  bookmarked: boolean
  // 总单元数（pdf 为页数）
  total: number
  // 当前翻页模式（滚动 / 翻页）
  turnMode: PageTurnMode
  // 当前版面（单页 / 双页对开）
  layout: PageLayout
}

// 创建引擎所需选项。getSource 返回文档原始字节，由外壳注入。
// chapters 由后端章节索引提供（txt/epub 用），location 为引擎自定义定位令牌：
// txt 为字符偏移字符串，epub 为 spine/锚点等。
// 持久化回调由外壳注入：onPositionChange 上报位置变化（jump=true 表示跳转类动作，
// 用于 3 槽自动进度开新槽）；addBookmark/removeBookmark 负责书签入库，addBookmark
// 返回后端生成的书签 id。回调缺省时书签仅存内存。
export interface EngineOptions {
  getSource: () => Promise<ArrayBuffer>
  chapters?: { chapter_no: number; title: string | null; location: string }[]
  onPositionChange?: (jump: boolean) => void
  addBookmark?: (contentCoord: string, label: string) => Promise<string>
  removeBookmark?: (id: string) => Promise<void>
}

export interface ReaderEngine {
  readonly format: BookFormat
  readonly state: ReaderState
  readonly actions: ReaderAction[]
  // 将引擎内容挂载到指定的滚动容器内
  mount(container: HTMLElement): Promise<void>
  goPrev(): void
  goNext(): void
  // 解析用户输入（pdf 为页码，txt/epub 可为百分比等），成功返回 true
  goToLocation(input: string): boolean
  goToToc(item: TocItem): void
  // 渲染任意页为截图 dataURL（仅 PDF 引擎实现），失败返回 null
  renderPageImage?(page: number): Promise<string | null>
  // 当前位置的内容坐标（字体无关，格式随格式而异：txt=章节号:章内字符偏移，
  // epub=CFI，pdf=页码:页内滚动比例），未就绪返回 null
  getContentCoord(): string | null
  // 跳转到内容坐标（用于进度/书签恢复，不算跳转类动作），无法解析返回 false
  goToContentCoord(coord: string): boolean
  // 在当前位置添加/移除书签
  toggleBookmark(): void
  removeBookmark(id: string): void
  goToBookmark(item: ReaderBookmark): void
  // 切换翻页模式（滚动 / 翻页）
  setPageTurnMode(mode: PageTurnMode): void
  // 切换版面（单页 / 双页对开）
  setPageLayout(layout: PageLayout): void
  destroy(): void
}
