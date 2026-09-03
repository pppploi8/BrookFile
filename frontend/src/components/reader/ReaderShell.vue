<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeft, ArrowRight, List, FullScreen, Loading, Setting, CollectionTag, Close } from '@element-plus/icons-vue'
import { ElMessage } from '@/utils/message'
import { createEngine } from '@/reader/engines'
import { ProgressTracker } from '@/reader/persistence'
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
const settingsOpen = ref(false)
const jumpInput = ref('')
const isFs = ref(false)

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
  document.addEventListener('fullscreenchange', onFsChange)
})

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
  document.removeEventListener('fullscreenchange', onFsChange)
})

function onFsChange(): void {
  isFs.value = !!document.fullscreenElement
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
    <div class="reader-toolbar">
      <el-button text :icon="ArrowLeft" @click="emit('back')" />
      <span class="reader-title" :title="title">{{ title }}</span>
      <el-tag size="small" effect="plain" class="format-tag">{{ format.toUpperCase() }}</el-tag>
      <div class="spacer" />
      <el-button v-if="state" text :title="t('reader.bookmarks')" @click="bookmarksOpen = !bookmarksOpen">
        <el-icon><CollectionTag /></el-icon>
      </el-button>
      <el-button v-if="showToc" text :title="t('reader.toc')" @click="tocOpen = !tocOpen">
        <el-icon><List /></el-icon>
      </el-button>
      <el-button text :title="t('reader.fullscreen')" @click="toggleFullscreen">
        <el-icon><FullScreen /></el-icon>
      </el-button>
      <el-popover
        v-if="state && state.ready"
        v-model:visible="settingsOpen"
        placement="bottom-end"
        :width="300"
        trigger="click"
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

      <div class="reader-viewport" ref="viewport">
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
    </div>

    <div class="reader-footer">
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
  display: flex;
}
.reader-toc {
  width: 240px;
  flex-shrink: 0;
  border-right: 1px solid var(--el-border-color-lighter);
  overflow: auto;
  padding: 8px;
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
  flex: 1;
  min-height: 0;
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
  width: 240px;
  flex-shrink: 0;
  border-left: 1px solid var(--el-border-color-lighter);
  overflow: auto;
  padding: 8px;
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
</style>
