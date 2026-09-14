<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeft, ArrowRight, List, FullScreen, Loading, Setting, CollectionTag, Close, ChatDotRound } from '@element-plus/icons-vue'
import { ElMessage } from '@/utils/message'
import { createEngine } from '@/reader/engines'
import { ProgressTracker } from '@/reader/persistence'
import AiAssistantPanel from './AiAssistantPanel.vue'
import type { BookFormat, PageLayout, PageTurnMode, ReaderBookmark, ReaderEngine, TocItem } from '@/reader/types'
import type { ChapterItem } from '@/api/ebook'
import { useEbookStore } from '@/stores/ebook'

const props = defineProps<{
  bookId: string
  title: string
  format: BookFormat
  getSource: () => Promise<ArrayBuffer>
  chapters?: ChapterItem[]
}>()
const emit = defineEmits<{ (e: 'back'): void }>()
const { t } = useI18n()

const ebookStore = useEbookStore()

const rootEl = ref<HTMLElement>()
const viewport = ref<HTMLElement>()
const engine = shallowRef<ReaderEngine>()

const tocOpen = ref(false)
const bookmarksOpen = ref(false)
const aiOpen = ref(false)
const settingsOpen = ref(false)
const isMobile = ref(false)
const jumpInput = ref('')
const isFs = ref(false)
// 全屏模式下点击阅读区域隐藏/恢复顶部标题栏与底部进度栏
const chromeHidden = ref(false)

const state = computed(() => engine.value?.state)
const actions = computed(() => engine.value?.actions ?? [])
const runActions = computed(() => actions.value.filter(a => a.run))
const choiceActions = computed(() => actions.value.filter(a => a.options))
const showToc = computed(() => !!state.value && state.value.toc.length > 0)

const LS_TURN = 'brookfile.reader.turnMode'
const LS_LAYOUT = 'brookfile.reader.layout'

// 3 槽自动进度追踪器（initPersistence 中创建）。
let tracker: ProgressTracker | null = null

// 「最近位置」列表：进度槽按最近写入排序，点击可跳回。
const recentSlots = ref<{ slot: number; coord: string; label: string }[]>([])

onMounted(async () => {
  const eng = createEngine(props.format, {
    getSource: props.getSource,
    chapters: props.chapters,
    onPositionChange: (jump) => tracker?.onPositionChange(jump),
    addBookmark: async (contentCoord, label) => {
      const resp = await ebookStore.addBookmark(props.bookId, contentCoord, label)
      return resp.id
    },
    removeBookmark: (id) => ebookStore.removeBookmark(id),
  })
  engine.value = eng
  if (viewport.value) await eng.mount(viewport.value)
  applySavedPrefs()
  void initPersistence(eng)
  checkMobile()
  window.addEventListener('resize', checkMobile)
  document.addEventListener('keydown', onKeydown)
  document.addEventListener('selectionchange', onSelChange)
  document.addEventListener('fullscreenchange', onFsChange)
})

function checkMobile(): void {
  isMobile.value = window.innerWidth <= 768
}

// 加载书签列表与进度槽：书签填充到引擎内存列表，进度恢复到最新槽位。
async function initPersistence(eng: ReaderEngine): Promise<void> {
  tracker = new ProgressTracker(
    () => eng.getContentCoord(),
    (slot, coord) => {
      recentSlots.value = [
        { slot, coord, label: eng.state.locationLabel || coord },
        ...recentSlots.value.filter((s) => s.slot !== slot),
      ]
      return ebookStore.saveProgress(props.bookId, slot, coord, eng.state.locationLabel || null)
    },
  )
  try {
    const [prog, bms] = await Promise.all([
      ebookStore.getProgress(props.bookId),
      ebookStore.listBookmarks(props.bookId),
    ])
    if (!eng.state.ready) return
    if (bms.success) {
      eng.state.bookmarks = bms.bookmarks.map((b) => ({
        id: b.id,
        label: b.summary ?? b.content_coord,
        location: b.content_coord,
      }))
    }
    if (prog.success) {
      recentSlots.value = [...prog.slots]
        .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
        .map((s) => ({ slot: s.slot, coord: s.content_coord, label: s.summary ?? s.content_coord }))
      const coord = tracker.applyLoadedSlots(prog.slots)
      if (coord) eng.goToContentCoord(coord)
    }
  } catch {
    // 加载/恢复失败不阻塞阅读，后续位置变化仍会正常保存
  }
}

function applySavedPrefs(): void {
  const eng = engine.value
  if (!eng) return
  const turn = localStorage.getItem(LS_TURN)
  const layout = localStorage.getItem(LS_LAYOUT)
  if (turn === 'scroll' || turn === 'paginated') eng.setPageTurnMode(turn)
  if (layout === 'single' || layout === 'double') eng.setPageLayout(layout)
}

function onTurn(mode: PageTurnMode): void {
  engine.value?.setPageTurnMode(mode)
  localStorage.setItem(LS_TURN, mode)
}

function onLayout(layout: PageLayout): void {
  engine.value?.setPageLayout(layout)
  localStorage.setItem(LS_LAYOUT, layout)
}

onUnmounted(() => {
  tracker?.dispose()
  tracker = null
  engine.value?.destroy()
  window.removeEventListener('resize', checkMobile)
  document.removeEventListener('keydown', onKeydown)
  document.removeEventListener('selectionchange', onSelChange)
  document.removeEventListener('fullscreenchange', onFsChange)
})

function onFsChange(): void {
  isFs.value = !!document.fullscreenElement
  if (!isFs.value) chromeHidden.value = false
}
// 点击阅读区域切换工具栏显隐（仅全屏模式；正在选取文本时不触发）
function onViewportTap(): void {
  if (!isFs.value) return
  const sel = window.getSelection()
  if (sel && sel.toString()) return
  chromeHidden.value = !chromeHidden.value
}
function toggleFullscreen(): void {
  if (!document.fullscreenElement) rootEl.value?.requestFullscreen?.()
  else document.exitFullscreen?.()
}
function onTocClick(item: TocItem): void {
  engine.value?.goToToc(item)
  tocOpen.value = false
}
function onBookmarkToggle(): void {
  engine.value?.toggleBookmark()
}
function toggleBookmarks(): void {
  bookmarksOpen.value = !bookmarksOpen.value
  aiOpen.value = false
}
function toggleAi(): void {
  aiOpen.value = !aiOpen.value
  bookmarksOpen.value = false
}

// —— 截图/AI 引用 —— 由 AI 侧边栏触发
// PDF：覆盖阅读区拖拽框选，截取 pdf.js 画布对应区域返回 dataURL
// EPUB/TXT：进入文本选择模式，返回用户选中的引用文本
const captureActive = ref(false)
const captureEl = ref<HTMLElement>()
const captureRect = ref({ x: 0, y: 0, w: 0, h: 0, active: false })
const referencing = ref(false)
const hasSelection = ref(false)
let captureResolve: ((v: string | null) => void) | null = null
let captureStart: { x: number; y: number } | null = null

// EPUB 正文渲染在 epubjs 的 iframe 内，选择需从 iframe 文档读取
function iframeDoc(): Document | null {
  if (props.format !== 'epub') return null
  const iframe = viewport.value?.querySelector('iframe') as HTMLIFrameElement | null
  return iframe?.contentDocument ?? null
}

function selDoc(): Document | null {
  return iframeDoc() ?? document
}

function selectionText(): string {
  return selDoc()?.getSelection()?.toString().trim() ?? ''
}

function hidePanel(): void {
  if (isMobile.value) aiOpen.value = false
  // 桌面端通过 css class（referencing/captureActive）隐藏，保持组件挂载以保留会话
}

function restorePanel(): void {
  if (isMobile.value) aiOpen.value = true
}

function provideCapture(): Promise<string | null> {
  hidePanel()
  return new Promise<string | null>((resolve) => {
    captureResolve = resolve
    if (props.format === 'pdf') {
      captureActive.value = true
    } else {
      referencing.value = true
      iframeDoc()?.addEventListener('selectionchange', onSelChange)
    }
  })
}

// AI 页面查询：按 kind 分发（不做用户交互，全部离屏处理）。
// pdf_page：渲染 PDF 页为截图，page 缺省时取当前阅读页，返回 JSON {page, image}
async function providePageQuery(kind: string, params: Record<string, unknown>): Promise<string | null> {
  if (kind === 'pdf_page') {
    if (props.format !== 'pdf') return null
    const bizId = typeof params.biz_id === 'string' ? params.biz_id : null
    if (bizId && bizId !== props.bookId) return null
    let page = typeof params.page === 'number' ? Math.floor(params.page) : 0
    if (page < 1) {
      const coord = engine.value?.getContentCoord()
      page = coord ? parseInt(coord.split(':')[0] ?? '', 10) : 0
    }
    if (page < 1) return null
    const image = await engine.value?.renderPageImage?.(page)
    if (!image) return null
    return JSON.stringify({ page, image })
  }
  return null
}

function cancelCapture(): void {
  captureStart = null
  captureRect.value = { x: 0, y: 0, w: 0, h: 0, active: false }
  captureActive.value = false
  captureResolve?.(null)
  restorePanel()
}

function onCaptureDown(e: PointerEvent): void {
  if (e.pointerType === 'mouse' && e.button !== 0) return
  if (!captureEl.value) return
  const r = captureEl.value.getBoundingClientRect()
  captureStart = { x: e.clientX - r.left, y: e.clientY - r.top }
  captureRect.value = { x: captureStart.x, y: captureStart.y, w: 0, h: 0, active: true }
}

function onCaptureMove(e: PointerEvent): void {
  if (!captureStart || !captureEl.value) return
  const r = captureEl.value.getBoundingClientRect()
  const x = e.clientX - r.left
  const y = e.clientY - r.top
  captureRect.value = {
    x: Math.min(captureStart.x, x),
    y: Math.min(captureStart.y, y),
    w: Math.abs(x - captureStart.x),
    h: Math.abs(y - captureStart.y),
    active: true,
  }
}

async function onCaptureUp(): Promise<void> {
  if (!captureStart) return
  const rect = captureRect.value
  captureStart = null
  if (rect.w < 8 || rect.h < 8) {
    cancelCapture()
    return
  }
  const vp = viewport.value
  const ol = captureEl.value
  if (!vp || !ol) {
    cancelCapture()
    return
  }
  try {
    const dataUrl = capturePdfRegion(rect, vp, ol)
    if (!dataUrl) throw new Error('empty')
    captureActive.value = false
    captureRect.value = { x: 0, y: 0, w: 0, h: 0, active: false }
    captureResolve?.(dataUrl)
    restorePanel()
  } catch {
    captureResolve?.(null)
    captureActive.value = false
    restorePanel()
  }
  captureResolve = null
}

// 从 pdf.js 渲染的画布中抠取选区像素，避免整页 DOM 截图
function capturePdfRegion(
  rect: { x: number; y: number; w: number; h: number },
  vp: HTMLElement,
  ol: HTMLElement,
): string | null {
  const olRect = ol.getBoundingClientRect()
  const dpr = window.devicePixelRatio || 1
  const out = document.createElement('canvas')
  out.width = Math.max(1, Math.round(rect.w * dpr))
  out.height = Math.max(1, Math.round(rect.h * dpr))
  const ctx = out.getContext('2d')
  if (!ctx) return null
  ctx.fillStyle = '#fff'
  ctx.fillRect(0, 0, out.width, out.height)
  ctx.scale(dpr, dpr)

  let hit = false
  const canvases = Array.from(vp.querySelectorAll('canvas'))
  for (const c of canvases) {
    const cr = c.getBoundingClientRect()
    const cx = cr.left - olRect.left
    const cy = cr.top - olRect.top
    const iw = Math.min(rect.x + rect.w, cx + cr.width) - Math.max(rect.x, cx)
    const ih = Math.min(rect.y + rect.h, cy + cr.height) - Math.max(rect.y, cy)
    if (iw <= 0 || ih <= 0) continue
    hit = true
    const sxScale = c.width / cr.width
    const syScale = c.height / cr.height
    ctx.drawImage(
      c,
      (Math.max(rect.x, cx) - cx) * sxScale,
      (Math.max(rect.y, cy) - cy) * syScale,
      iw * sxScale,
      ih * syScale,
      Math.max(rect.x, cx) - rect.x,
      Math.max(rect.y, cy) - rect.y,
      iw,
      ih,
    )
  }
  return hit ? out.toDataURL('image/png') : null
}

function onSelChange(): void {
  if (!referencing.value) return
  hasSelection.value = selectionText().length > 0
}

function confirmReference(): void {
  const text = selectionText()
  if (!text) {
    cancelReference()
    return
  }
  selDoc()?.getSelection()?.removeAllRanges()
  referencing.value = false
  hasSelection.value = false
  restorePanel()
  captureResolve?.(text)
  captureResolve = null
}

function cancelReference(): void {
  selDoc()?.getSelection()?.removeAllRanges()
  referencing.value = false
  hasSelection.value = false
  restorePanel()
  captureResolve?.(null)
  captureResolve = null
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    if (captureActive.value) cancelCapture()
    else if (referencing.value) cancelReference()
  }
}
function onBookmarkClick(item: ReaderBookmark): void {
  engine.value?.goToBookmark(item)
  bookmarksOpen.value = false
}
function onBookmarkRemove(item: ReaderBookmark): void {
  engine.value?.removeBookmark(item.id)
}
function onRecentClick(s: { slot: number; coord: string }): void {
  const ok = engine.value?.goToContentCoord(s.coord)
  if (ok) tracker?.onPositionChange(true)
  bookmarksOpen.value = false
}
function onJump(): void {
  const v = jumpInput.value.trim()
  if (!v) return
  const ok = engine.value?.goToLocation(v)
  if (!ok) ElMessage.warning({ __key: 'reader.invalidLocation' })
  jumpInput.value = ''
}
</script>

<template>
  <div class="reader-shell" ref="rootEl">
    <div v-show="!chromeHidden" class="reader-toolbar">
      <el-button text :icon="ArrowLeft" @click="emit('back')" />
      <span class="reader-title" :title="title">{{ title }}</span>
      <el-tag size="small" effect="plain" class="format-tag">{{ format.toUpperCase() }}</el-tag>
      <div class="spacer" />
      <el-button v-if="state" text :title="t('reader.bookmarks')" @click="toggleBookmarks">
        <el-icon><CollectionTag /></el-icon>
      </el-button>
      <el-button v-if="showToc" text :title="t('reader.toc')" @click="tocOpen = !tocOpen">
        <el-icon><List /></el-icon>
      </el-button>
      <el-button text :title="t('reader.fullscreen')" @click="toggleFullscreen">
        <el-icon><FullScreen /></el-icon>
      </el-button>
      <el-popover
        v-if="isMobile && (state?.ready ?? false)"
        v-model:visible="aiOpen"
        placement="bottom-end"
        :width="300"
        trigger="click"
        :teleported="false"
        popper-class="reader-ai-popover"
        @click="bookmarksOpen = false"
      >
        <template #reference>
          <el-button text :title="t('reader.aiAssistant')">
            <el-icon><ChatDotRound /></el-icon>
          </el-button>
        </template>
        <AiAssistantPanel :format="format" :book-id="bookId" :capture="provideCapture" :page-query="providePageQuery" />
      </el-popover>
      <el-button v-else text :title="t('reader.aiAssistant')" @click="toggleAi">
        <el-icon><ChatDotRound /></el-icon>
      </el-button>
      <el-popover
        v-if="state && state.ready"
        v-model:visible="settingsOpen"
        placement="bottom-end"
        :width="300"
        trigger="click"
        :teleported="false"
        popper-class="reader-settings-popover"
      >
        <template #reference>
          <el-button text :title="t('reader.settings')">
            <el-icon><Setting /></el-icon>
          </el-button>
        </template>
        <div class="settings-panel">
          <div class="settings-row">
            <span class="settings-label">{{ t('reader.modeLabel') }}</span>
            <el-button-group>
              <el-button size="small" :type="state.turnMode === 'scroll' ? 'primary' : ''" @click="onTurn('scroll')">{{ t('reader.scrollMode') }}</el-button>
              <el-button size="small" :type="state.turnMode === 'paginated' ? 'primary' : ''" @click="onTurn('paginated')">{{ t('reader.paginatedMode') }}</el-button>
            </el-button-group>
          </div>
          <div class="settings-row">
            <span class="settings-label">{{ t('reader.layoutLabel') }}</span>
            <el-button-group>
              <el-button size="small" :type="state.layout === 'single' ? 'primary' : ''" @click="onLayout('single')">{{ t('reader.singlePage') }}</el-button>
              <el-button size="small" :type="state.layout === 'double' ? 'primary' : ''" @click="onLayout('double')">{{ t('reader.doublePage') }}</el-button>
            </el-button-group>
          </div>
          <div v-if="runActions.length" class="settings-row">
            <span class="settings-label">{{ format === 'pdf' ? t('reader.zoomLabel') : t('reader.contentSizeLabel') }}</span>
            <el-button
              v-for="a in runActions"
              :key="a.key"
              text
              :title="t(a.title)"
              @click="a.run?.()"
            >
              <el-icon><component :is="a.icon" /></el-icon>
            </el-button>
          </div>
          <div v-for="a in choiceActions" :key="a.key" class="settings-row">
            <span class="settings-label">{{ t(a.title) }}</span>
            <div class="settings-options">
              <el-button
                v-for="o in a.options?.() ?? []"
                :key="o.key"
                size="small"
                :type="o.active ? 'primary' : ''"
                plain
                @click="a.select?.(o.key)"
              >
                <span v-if="o.color" class="theme-dot" :style="{ background: o.color }" />
                <span>{{ o.text ?? (o.label ? t(o.label) : '') }}</span>
              </el-button>
            </div>
          </div>
        </div>
      </el-popover>
    </div>

    <div class="reader-body">
      <div v-if="showToc && tocOpen" class="reader-toc">
        <div
          v-for="item in state?.toc ?? []"
          :key="item.id"
          class="toc-item"
          :style="{ paddingLeft: 8 + (item.level || 0) * 14 + 'px' }"
          @click="onTocClick(item)"
        >
          {{ item.label }}
        </div>
      </div>

      <div class="reader-viewport" ref="viewport" @click="onViewportTap">
        <div v-if="!state || state.loading" class="reader-center">
          <el-icon class="is-loading" :size="28"><Loading /></el-icon>
          <span class="center-text">{{ t('reader.loading') }}</span>
        </div>
        <div v-else-if="state.error" class="reader-center">
          <el-result icon="error" :title="t('reader.loadError')" :sub-title="state.error" />
        </div>
      </div>

      <div v-if="bookmarksOpen" class="reader-bookmarks">
        <template v-if="recentSlots.length">
          <div class="panel-section-title">{{ t('reader.recentPositions') }}</div>
          <div
            v-for="s in recentSlots"
            :key="s.slot"
            class="bm-item recent-item"
            @click="onRecentClick(s)"
          >
            <span class="bm-label" :title="s.label">{{ s.label }}</span>
          </div>
        </template>
        <div class="panel-section-title">{{ t('reader.bookmarks') }}</div>
        <el-button
          v-if="state"
          class="bm-add"
          size="small"
          type="primary"
          plain
          :disabled="state.bookmarked"
          @click="onBookmarkToggle"
        >
          {{ t('reader.addBookmark') }}
        </el-button>
        <div v-if="!state?.bookmarks.length" class="bm-empty">{{ t('reader.noBookmarks') }}</div>
        <div
          v-for="b in state?.bookmarks ?? []"
          :key="b.id"
          class="bm-item bookmark-item"
          @click="onBookmarkClick(b)"
        >
          <span class="bm-label" :title="b.label">{{ b.label }}</span>
          <el-icon class="bm-del" @click.stop="onBookmarkRemove(b)"><Close /></el-icon>
        </div>
      </div>

      <div v-if="!isMobile && aiOpen" class="reader-ai" :class="{ 'reader-ai-hidden': referencing || captureActive }">
        <AiAssistantPanel :format="format" :book-id="bookId" :capture="provideCapture" :page-query="providePageQuery" />
      </div>

      <div
        v-if="referencing"
        class="reference-bar"
        @mousedown.prevent
        @pointerdown.prevent
      >
        <span>{{ t('reader.aiRefHint') }}</span>
        <el-button
          size="small"
          type="primary"
          :disabled="!hasSelection"
          @click="confirmReference"
        >
          {{ t('reader.aiRefSend') }}
        </el-button>
        <el-button size="small" text @click="cancelReference">{{ t('common.cancel') }}</el-button>
      </div>

      <div
        v-if="captureActive"
        ref="captureEl"
        class="capture-overlay"
        @pointerdown="onCaptureDown"
        @pointermove="onCaptureMove"
        @pointerup="onCaptureUp"
        @contextmenu.prevent
      >
        <div class="capture-hint">
          <span>{{ t('reader.aiCaptureHint') }}</span>
          <el-button text size="small" @click.stop="cancelCapture">{{ t('common.cancel') }}</el-button>
        </div>
        <div
          v-if="captureRect.active"
          class="capture-box"
          :style="{
            left: captureRect.x + 'px',
            top: captureRect.y + 'px',
            width: captureRect.w + 'px',
            height: captureRect.h + 'px',
          }"
        />
      </div>
    </div>

    <div v-show="!chromeHidden" class="reader-footer">
      <el-button text :disabled="!state || !state.canPrev" @click="engine?.goPrev()">
        <el-icon><ArrowLeft /></el-icon>
      </el-button>
      <span class="loc-label">{{ state?.locationLabel || '' }}</span>
      <el-input
        v-model="jumpInput"
        class="jump-input"
        size="small"
        :placeholder="t('reader.jump')"
        @keyup.enter="onJump"
      />
      <el-button text :disabled="!state || !state.canNext" @click="engine?.goNext()">
        <el-icon><ArrowRight /></el-icon>
      </el-button>
      <el-progress
        class="reader-progress"
        :percentage="Math.round((state?.progress || 0) * 100)"
        :show-text="false"
      />
    </div>
  </div>
</template>

<style scoped>
.reader-shell {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  flex-direction: column;
  background: var(--el-bg-color);
}
.reader-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.reader-title {
  font-weight: 600;
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.spacer {
  flex: 1;
}
.settings-panel {
  display: flex;
  flex-direction: column;
}
.settings-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 2px;
}
.settings-label {
  flex-shrink: 0;
  min-width: 56px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.settings-options {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.theme-dot {
  width: 14px;
  height: 14px;
  border-radius: 4px;
  border: 1px solid var(--el-border-color-darker);
  margin-right: 4px;
  flex-shrink: 0;
}
.reader-toolbar .el-button + .el-button {
  margin-left: 0;
}
.format-tag {
  font-family: monospace;
}
.reader-body {
  flex: 1;
  min-height: 0;
  position: relative;
  overflow: hidden;
}
.reader-toc {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  z-index: 5;
  width: 240px;
  border-right: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
  overflow: auto;
  padding: 8px;
  box-shadow: 2px 0 8px rgba(0, 0, 0, 0.06);
}
.toc-item {
  padding: 6px 8px;
  cursor: pointer;
  border-radius: 4px;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toc-item:hover {
  background: var(--el-fill-color-light);
}
.reader-viewport {
  position: absolute;
  inset: 0;
  overflow: auto;
  padding: 12px;
  background: var(--el-fill-color-blank);
}
.reader-center {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 60vh;
  gap: 10px;
  color: var(--el-text-color-secondary);
}
.center-text {
  font-size: 13px;
}
.pdf-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 100%;
}
.pdf-spread {
  display: flex;
  flex-direction: row;
  flex-wrap: nowrap;
  justify-content: center;
  align-items: flex-start;
  gap: 16px;
  width: 100%;
}
.pdf-page {
  background: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex-shrink: 0;
}
.reader-bookmarks {
  position: absolute;
  top: 0;
  bottom: 0;
  right: 0;
  z-index: 5;
  width: 240px;
  border-left: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
  overflow: auto;
  padding: 8px;
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.06);
}
.reader-ai {
  position: absolute;
  top: 0;
  bottom: 0;
  right: 0;
  z-index: 5;
  width: 320px;
  border-left: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
  overflow: auto;
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.06);
}
.reader-ai-hidden {
  display: none;
}
.capture-overlay {
  position: absolute;
  inset: 0;
  z-index: 30;
  background: rgba(0, 0, 0, 0.05);
  cursor: crosshair;
  touch-action: none;
}
.reference-bar {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 30;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.12);
  font-size: 13px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}
.capture-hint {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}
.capture-box {
  position: absolute;
  border: 1.5px solid var(--el-color-primary);
  background: rgba(64, 158, 255, 0.12);
  pointer-events: none;
}
.panel-section-title {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 4px 8px 6px;
}
.recent-item + .panel-section-title {
  margin-top: 8px;
}
.bm-add {
  width: 100%;
  margin-bottom: 8px;
}
.bm-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  cursor: pointer;
  border-radius: 4px;
  font-size: 13px;
}
.bm-item:hover {
  background: var(--el-fill-color-light);
}
.bm-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bm-del {
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}
.bm-del:hover {
  color: var(--el-color-danger);
}
.bm-empty {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  padding: 8px;
  text-align: center;
}
.reader-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-top: 1px solid var(--el-border-color-lighter);
}
.jump-input {
  width: 90px;
}
.loc-label {
  min-width: 70px;
  text-align: center;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.reader-progress {
  flex: 1;
}
</style>

<style>
.reader-settings-popover .el-button + .el-button {
  margin-left: 0;
}
.reader-ai-popover {
  height: 70vh;
  height: 70dvh;
  padding: 0;
  overflow: hidden;
}
</style>
