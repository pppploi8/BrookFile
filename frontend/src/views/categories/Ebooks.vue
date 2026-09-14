<template>
  <div class="ebooks-container" :class="{ 'is-mobile': isMobileLayout }">
    <aside class="category-panel">
      <div class="category-list">
        <div
          class="category-item"
          :class="{ active: activeCategory === 'all' }"
          @click="activeCategory = 'all'"
        >
          <span class="category-name">{{ t('ebook.allBooks') }}</span>
          <span class="category-count">{{ books.length }}</span>
        </div>
        <div
          class="category-item"
          :class="{ active: activeCategory === 'root' }"
          @click="activeCategory = 'root'"
        >
          <span class="category-name">{{ t('ebook.uncategorized') }}</span>
          <span class="category-count">{{ countInCategory(null) }}</span>
        </div>
        <div
          v-for="cat in categories"
          :key="cat.id"
          class="category-item"
          :class="{ active: activeCategory === cat.id }"
          @click="activeCategory = cat.id"
        >
          <span class="category-name">{{ cat.name }}</span>
          <span class="category-count">{{ countInCategory(cat.id) }}</span>
          <el-dropdown
            trigger="click"
            @command="(cmd: string) => handleCategoryCommand(cmd, cat)"
            @click.stop
          >
            <span class="category-more" @click.stop>
              <el-icon><MoreFilled /></el-icon>
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="rename">{{ t('ebook.renameCategory') }}</el-dropdown-item>
                <el-dropdown-item command="delete">{{ t('ebook.deleteCategory') }}</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>
      <div class="category-footer">
        <el-button text type="primary" :icon="Plus" @click="handleAddCategory">
          {{ t('ebook.newCategory') }}
        </el-button>
      </div>
    </aside>

    <section class="shelf-panel">
      <div class="shelf-toolbar">
        <div class="toolbar-title">
          {{ currentFilterName }}
          <span class="book-count">{{ t('ebook.bookCount', { count: filteredBooks.length }) }}</span>
        </div>
        <el-dropdown trigger="click" @command="handleAddCommand">
          <el-button type="primary">
            <el-icon class="btn-icon"><Plus /></el-icon>
            {{ t('ebook.addBook') }}
            <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="file">
                <el-icon><Document /></el-icon>
                <span>{{ t('ebook.addFromFile') }}</span>
              </el-dropdown-item>
              <el-dropdown-item command="scan">
                <el-icon><FolderOpened /></el-icon>
                <span>{{ t('ebook.scanDirectory') }}</span>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>

      <div class="shelf-grid-wrapper">
        <el-empty v-if="filteredBooks.length === 0" :description="t('ebook.emptyShelf')" />
        <div v-else class="shelf-grid">
          <div
            v-for="book in filteredBooks"
            :key="book.id"
            class="book-card"
            @click="handleOpenBook(book)"
          >
            <div class="book-cover" :style="coverStyle(book)">
              <img v-if="previewUrl(book.id)" :src="previewUrl(book.id)" class="cover-image" />
              <span v-else class="cover-title">{{ book.title }}</span>
              <span class="format-badge">{{ book.format.toUpperCase() }}</span>
              <span v-if="ebookStore.isCached(book.id)" class="cached-badge" :title="t('ebook.cached')">
                <el-icon><CircleCheckFilled /></el-icon>
              </span>
              <div
                v-if="book.scan_status === 'pending' || book.scan_status === 'scanning'"
                class="cover-overlay"
              >
                <el-icon class="cover-spinner"><Loading /></el-icon>
                <span class="overlay-text">{{ t('ebook.scanningMeta') }}</span>
              </div>
            </div>
            <div class="book-info">
              <div class="book-title" :title="book.title">{{ book.title }}</div>
              <div class="book-meta">
                <span>{{ formatFileSize(book.file_size) }}</span>
              </div>
              <div class="book-added" :title="`${t('ebook.addedAt')} ${book.added_at}`">{{ t('ebook.addedAt') }} {{ book.added_at.split(' ')[0] }}</div>
              <div class="book-progress">{{ progressLine(book.id) }}</div>
              <div class="book-actions" @click.stop>
                <el-button text bg size="small" @click="openMoveDialog(book)">
                  <el-icon><Folder /></el-icon>
                  <span>{{ t('ebook.move') }}</span>
                </el-button>
                <el-button text bg size="small" type="danger" @click="handleRemoveBook(book)">
                  <el-icon><Delete /></el-icon>
                  <span>{{ t('ebook.remove') }}</span>
                </el-button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <el-dialog
      v-model="pickerVisible"
      :title="pickerMode === 'dir' ? t('ebook.selectDirectory') : t('ebook.selectFile')"
      :width="pickerDialogWidth"
      class="picker-dialog"
    >
      <div class="picker-breadcrumb">
        <el-link :underline="false" @click="pickerGotoRoot">{{ t('ebook.rootDirectory') }}</el-link>
        <template v-for="(part, idx) in pickerParts" :key="idx">
          <span class="breadcrumb-sep">/</span>
          <el-link :underline="false" @click="pickerGoto(idx)">{{ part }}</el-link>
        </template>
      </div>
      <div class="picker-list">
        <div v-if="pickerLoading" class="picker-loading">
          <el-icon class="scan-spinner"><Loading /></el-icon>
        </div>
        <template v-else>
          <div
            v-for="entry in pickerEntries"
            :key="entry.name"
            class="picker-entry"
            :class="{ disabled: entry.file_type !== 'directory' && (pickerMode === 'dir' || !fileFormat(entry.name)) }"
            @click="handlePickerEntry(entry)"
          >
            <el-icon class="picker-icon" :class="{ 'is-folder': entry.file_type === 'directory' }">
              <Folder v-if="entry.file_type === 'directory'" />
              <Document v-else />
            </el-icon>
            <span class="picker-name">{{ entry.name }}</span>
            <span v-if="entry.file_type === 'file'" class="picker-size">{{ formatFileSize(entry.size) }}</span>
          </div>
          <el-empty v-if="pickerEntries.length === 0" :image-size="48" :description="t('ebook.emptyDirectory')" />
        </template>
      </div>
      <template #footer>
        <el-button @click="pickerVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button v-if="pickerMode === 'dir'" type="primary" @click="handlePickDirectory">
          {{ t('ebook.selectThisDirectory') }}
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="scanVisible"
      :title="t('ebook.scanTitle')"
      :width="scanDialogWidth"
      :close-on-click-modal="false"
    >
      <template v-if="scanPhase === 'discover'">
        <div class="scan-status">
          <el-icon v-if="scanDiscovering" class="scan-spinner"><Loading /></el-icon>
          <span v-if="scanDiscovering">
            {{ t('ebook.scanning') }} {{ scanDir || '/' }} · {{ t('ebook.discoveredCount', { count: discovered.length }) }}
          </span>
          <span v-else>{{ t('ebook.scanFinished', { count: discovered.length }) }}</span>
        </div>
        <div class="scan-list">
          <div v-for="item in discovered" :key="item.path" class="scan-entry">
            <el-checkbox
              v-model="item.checked"
              @click.stop
            />
            <span class="scan-format">{{ item.format.toUpperCase() }}</span>
            <span class="scan-name" :title="item.path">{{ item.title }}</span>
            <span class="scan-path">{{ item.path }}</span>
            <span class="scan-size">{{ formatFileSize(item.file_size) }}</span>
          </div>
        </div>
        <div class="scan-footer-bar">
          <el-checkbox v-model="scanSelectAll">{{ t('ebook.selectAll') }}</el-checkbox>
          <span class="scan-selected">{{ t('ebook.selectedCount', { count: checkedCount }) }}</span>
        </div>
      </template>
      <template v-else>
        <div class="scan-status">
          <el-icon v-if="scanRunning" class="scan-spinner"><Loading /></el-icon>
          <span v-if="scanRunning">
            {{ t('ebook.importing') }} ({{ scanCompleted }} / {{ scanTotal }})
          </span>
          <span v-else>{{ t('ebook.importFinished', { count: scanAddedCount }) }}</span>
        </div>
        <div v-if="scanCurrent && scanRunning" class="scan-current">{{ scanCurrent }}</div>
        <el-progress :percentage="scanPercent" :stroke-width="8" />
        <div v-if="scanDuplicates > 0" class="scan-dup">
          {{ t('ebook.duplicateCount', { count: scanDuplicates }) }}
        </div>
      </template>
      <template #footer>
        <template v-if="scanPhase === 'discover'">
          <el-button @click="scanVisible = false">{{ t('common.cancel') }}</el-button>
          <el-button type="primary" :disabled="checkedCount === 0 || !scanFinished" @click="handleImport">
            {{ t('ebook.import') }}
          </el-button>
        </template>
        <template v-else>
          <el-button type="primary" :disabled="scanRunning" @click="scanVisible = false">
            {{ t('common.confirm') }}
          </el-button>
        </template>
      </template>
    </el-dialog>

    <el-dialog
      v-model="downloadVisible"
      :title="t('ebook.downloadTitle')"
      :width="downloadDialogWidth"
      :close-on-click-modal="false"
      :show-close="false"
    >
      <div class="download-body">
        <div class="download-title">{{ downloadBook?.title }}</div>
        <el-progress :percentage="downloadPercent" :stroke-width="10" />
        <div class="download-bytes">
          {{ formatFileSize(downloadLoaded) }} / {{ formatFileSize(downloadTotal) }}
        </div>
      </div>
      <template #footer>
        <el-button @click="cancelDownload">{{ t('common.cancel') }}</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="moveVisible" :title="t('ebook.moveToCategory')" :width="moveDialogWidth">
      <el-radio-group v-model="moveTarget" class="move-radio-group">
        <el-radio value="root">{{ t('ebook.uncategorized') }}</el-radio>
        <el-radio v-for="cat in categories" :key="cat.id" :value="cat.id">{{ cat.name }}</el-radio>
      </el-radio-group>
      <template #footer>
        <el-button @click="moveVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleMoveConfirm">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="exceptionVisible"
      :title="exceptionState?.sourceStatus === 'content_changed' ? t('ebook.srcChangedTitle') : t('ebook.srcMissingTitle')"
      :width="exceptionDialogWidth"
    >
      <p class="exception-desc">
        {{
          exceptionState?.sourceStatus === 'content_changed'
            ? t('ebook.srcChangedDesc', { title: exceptionState?.title ?? '' })
            : t('ebook.srcMissingDesc', { title: exceptionState?.title ?? '', path: exceptionState?.sourcePath ?? '' })
        }}
      </p>
      <template #footer>
        <el-button @click="exceptionVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="danger" @click="handleExceptionDelete">{{ t('ebook.deleteBook') }}</el-button>
        <el-button v-if="exceptionState?.sourceStatus === 'content_changed'" type="primary" @click="handleUpdateMeta">
          {{ t('ebook.updateMeta') }}
        </el-button>
        <el-button v-else type="primary" @click="handleRelink">
          {{ t('ebook.relinkFile') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessageBox } from 'element-plus'
import { ElMessage } from '@/utils/message'
import {
  Plus,
  ArrowDown,
  MoreFilled,
  Document,
  Folder,
  FolderOpened,
  Loading,
  CircleCheckFilled,
  Delete,
} from '@element-plus/icons-vue'
import { useEbookStore } from '@/stores/ebook'
import { useUserStore } from '@/stores/user'
import {
  ebookPreviewGet,
  ebookBookDownload,
  ebookShelfCover,
  type ShelfBook,
  type ShelfCategory,
  type SourceStatus,
  type EbookFormat,
} from '@/api/ebook'
import { renderPdfFirstPageCover } from '@/utils/pdfCover'
import { browseFiles, type FileItem } from '@/api/system'
import { formatFileSize } from '@/utils/format'
import { downloadWithProgress, getCachedBookData, cacheBookData, isBookCached } from '@/utils/ebookCache'

const { t } = useI18n()
const router = useRouter()
const ebookStore = useEbookStore()
const userStore = useUserStore()

const isMobileLayout = ref(false)
const checkLayout = () => {
  const width = window.innerWidth
  const height = window.innerHeight
  isMobileLayout.value = height / width > 1.2 || width < 768
}

const books = computed(() => ebookStore.books)
const categories = computed(() => ebookStore.categories)
const activeCategory = ref<'all' | 'root' | string>('all')

const countInCategory = (id: string | null) => books.value.filter(b => b.category_id === id).length

const filteredBooks = computed(() => {
  let list = books.value
  if (activeCategory.value === 'root') {
    list = list.filter(b => b.category_id === null)
  } else if (activeCategory.value !== 'all') {
    list = list.filter(b => b.category_id === activeCategory.value)
  }
  return [...list].sort((a, b) => b.added_at.localeCompare(a.added_at))
})

const currentFilterName = computed(() => {
  if (activeCategory.value === 'all') return t('ebook.allBooks')
  if (activeCategory.value === 'root') return t('ebook.uncategorized')
  return categories.value.find(c => c.id === activeCategory.value)?.name ?? ''
})

const titleHue = (title: string): number => {
  let h = 0
  for (let i = 0; i < title.length; i++) {
    h = (h * 31 + title.charCodeAt(i)) % 360
  }
  return h
}

const coverStyle = (book: ShelfBook) => {
  const hue = titleHue(book.title)
  return {
    background: `linear-gradient(160deg, hsl(${hue}, 55%, 46%) 0%, hsl(${(hue + 40) % 360}, 60%, 28%) 100%)`,
  }
}

const previewUrls = reactive(new Map<string, string>())
const previewLoading = new Set<string>()

const previewUrl = (id: string): string | undefined => previewUrls.get(id)

// 前端懒生成并上传 PDF 封面的并发控制（避免一次性下载全部 PDF）。
const coverGen = new Set<string>()
const coverQueue: ShelfBook[] = []
let coverRunning = 0
const COVER_CONCURRENCY = 2

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
}

// 用 pdf.js 在浏览器端渲染 PDF 首页为压缩 JPEG，并上传给后端作为封面。
async function generateAndUploadCover(book: ShelfBook) {
  if (book.format !== 'pdf' || book.has_preview) return
  try {
    const buf = await ebookBookDownload(book.id)
    const blob = await renderPdfFirstPageCover(buf)
    if (!blob) return
    const dataUrl = await blobToDataUrl(blob)
    const res = await ebookShelfCover(book.id, dataUrl)
    if (res.success) {
      book.has_preview = true
      await ensurePreview(book)
    }
  } catch {
    // 封面生成为尽力而为，失败不影响阅读
  }
}

async function processCoverQueue() {
  while (coverRunning < COVER_CONCURRENCY && coverQueue.length > 0) {
    const book = coverQueue.shift()!
    if (coverGen.has(book.id)) continue
    coverGen.add(book.id)
    coverRunning++
    generateAndUploadCover(book).finally(() => {
      coverRunning--
      coverGen.delete(book.id)
      processCoverQueue()
    })
  }
}

async function ensurePreview(book: ShelfBook) {
  if (book.has_preview) {
    if (previewUrls.has(book.id) || previewLoading.has(book.id)) return
    previewLoading.add(book.id)
    try {
      const blob = await ebookPreviewGet(book.id)
      if (blob.size > 0) previewUrls.set(book.id, URL.createObjectURL(blob))
    } catch {
      // preview unavailable
    } finally {
      previewLoading.delete(book.id)
    }
    return
  }
  // PDF 尚无封面：交给前端懒生成并上传（本地下载打开一次后再延迟显示）。
  if (book.format === 'pdf' && !coverGen.has(book.id) && !coverQueue.some((b) => b.id === book.id)) {
    coverQueue.push(book)
    processCoverQueue()
  }
}

watch(filteredBooks, (list) => {
  list.forEach(ensurePreview)
})

watch(books, (newBooks) => {
  const ids = new Set(newBooks.map(b => b.id))
  for (const [id, url] of previewUrls) {
    if (!ids.has(id)) {
      URL.revokeObjectURL(url)
      previewUrls.delete(id)
    }
  }
})

const fileFormat = (name: string): EbookFormat | null => {
  const dot = name.lastIndexOf('.')
  if (dot < 0) return null
  const ext = name.slice(dot + 1).toLowerCase()
  if (ext === 'txt' || ext === 'epub' || ext === 'pdf') return ext
  return null
}

const progressLabels = reactive(new Map<string, string>())

// 已查询过进度的书籍（无记录的书据此显示「尚未阅读」，查询中留空占位）。
const progressLoaded = reactive(new Set<string>())

function progressLine(id: string): string {
  const label = progressLabels.get(id)
  if (label) return `${t('ebook.readProgress')} ${label}`
  return progressLoaded.has(id) ? t('ebook.notRead') : ''
}

async function loadProgressLabels(ids: string[]) {
  await Promise.all(ids.map(async (id) => {
    if (progressLabels.has(id) || progressLoaded.has(id)) return
    try {
      const resp = await ebookStore.getProgress(id)
      if (!resp.success) return
      progressLoaded.add(id)
      if (resp.slots.length === 0) return
      const latest = [...resp.slots].sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0]
      if (latest?.summary) progressLabels.set(id, latest.summary)
    } catch {
      // progress is optional
    }
  }))
}

watch(books, (newBooks) => {
  const ids = new Set(newBooks.map(b => b.id))
  for (const id of progressLabels.keys()) {
    if (!ids.has(id)) progressLabels.delete(id)
  }
  for (const id of progressLoaded) {
    if (!ids.has(id)) progressLoaded.delete(id)
  }
  loadProgressLabels(newBooks.filter(b => !progressLabels.has(b.id) && !progressLoaded.has(b.id)).map(b => b.id))
})

const handleAddCategory = () => {
  ElMessageBox.prompt(t('ebook.categoryNamePrompt'), t('ebook.newCategory'), {
    confirmButtonText: t('common.confirm'),
    cancelButtonText: t('common.cancel'),
    inputPattern: /\S+/,
    inputErrorMessage: t('ebook.categoryNamePrompt'),
  }).then(async (res) => {
    const name = (res as { value: string }).value.trim()
    if (categories.value.some(c => c.name === name)) {
      ElMessage.warning({ __key: 'ebook.categoryNameExists' })
      return
    }
    try {
      await ebookStore.addCategory(name)
      ElMessage.success({ __key: 'ebook.categoryCreated' })
    } catch {
      // error already reported
    }
  }).catch(() => {})
}

const handleCategoryCommand = (cmd: string, cat: ShelfCategory) => {
  if (cmd === 'rename') {
    ElMessageBox.prompt(t('ebook.categoryNamePrompt'), t('ebook.renameCategory'), {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      inputValue: cat.name,
      inputPattern: /\S+/,
      inputErrorMessage: t('ebook.categoryNamePrompt'),
    }).then(async (res) => {
      const name = (res as { value: string }).value.trim()
      if (name !== cat.name && categories.value.some(c => c.name === name)) {
        ElMessage.warning({ __key: 'ebook.categoryNameExists' })
        return
      }
      try {
        await ebookStore.renameCategory(cat.id, name)
        ElMessage.success({ __key: 'ebook.categoryRenamed' })
      } catch {
        // error already reported
      }
    }).catch(() => {})
  } else if (cmd === 'delete') {
    ElMessageBox.confirm(
      t('ebook.deleteCategoryConfirm', { name: cat.name }),
      t('common.confirm'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
      }
    ).then(async () => {
      try {
        await ebookStore.removeCategory(cat.id)
        if (activeCategory.value === cat.id) activeCategory.value = 'all'
        ElMessage.success({ __key: 'ebook.categoryDeleted' })
      } catch {
        // error already reported
      }
    }).catch(() => {})
  }
}

const pickerVisible = ref(false)
const pickerMode = ref<'file' | 'dir'>('file')
const pickerPath = ref('')
const pickerEntries = ref<FileItem[]>([])
const pickerLoading = ref(false)
let pickerCallback: ((path: string) => void) | null = null

const pickerParts = computed(() => pickerPath.value.split('/').filter(Boolean))

async function loadPicker() {
  pickerLoading.value = true
  try {
    const resp = await browseFiles(pickerPath.value || undefined)
    pickerEntries.value = resp.files
  } catch {
    pickerEntries.value = []
  } finally {
    pickerLoading.value = false
  }
}

const openPicker = (mode: 'file' | 'dir', callback: (path: string) => void) => {
  pickerMode.value = mode
  pickerPath.value = ''
  pickerCallback = callback
  pickerVisible.value = true
  loadPicker()
}

const pickerGotoRoot = () => {
  pickerPath.value = ''
  loadPicker()
}

const pickerGoto = (idx: number) => {
  pickerPath.value = pickerParts.value.slice(0, idx + 1).join('/')
  loadPicker()
}

const handlePickerEntry = (entry: FileItem) => {
  if (entry.file_type === 'directory') {
    pickerPath.value = pickerPath.value ? `${pickerPath.value}/${entry.name}` : entry.name
    loadPicker()
    return
  }
  if (entry.file_type !== 'file') return
  if (pickerMode.value === 'dir') return
  if (!fileFormat(entry.name)) return
  const full = pickerPath.value ? `${pickerPath.value}/${entry.name}` : entry.name
  pickerVisible.value = false
  pickerCallback?.(full)
}

const handlePickDirectory = () => {
  const full = pickerPath.value
  pickerVisible.value = false
  pickerCallback?.(full)
}

const handleAddCommand = (cmd: string) => {
  if (cmd === 'file') {
    openPicker('file', async (path) => {
      try {
        const resp = await ebookStore.addBooks([{ path }])
        if (resp.duplicates.length > 0) {
          ElMessage.warning({ __key: 'ebook.duplicateBook' })
        } else if (resp.added.length > 0) {
          const first = resp.added[0]
          if (first) ElMessage.success({ __key: 'ebook.bookAdded', __params: { title: first.title } })
        }
        if (resp.batch_id) {
          await ebookStore.waitForScan(resp.batch_id)
          await ebookStore.loadShelf()
        }
      } catch {
        // error already reported
      }
    })
  } else if (cmd === 'scan') {
    openPicker('dir', (path) => startScan(path))
  }
}

interface DiscoverItem {
  path: string
  title: string
  format: EbookFormat
  file_size: number
  checked: boolean
}

const scanVisible = ref(false)
const scanPhase = ref<'discover' | 'import'>('discover')
const scanDir = ref('')
const scanDiscovering = ref(false)
const scanFinished = ref(false)
const discovered = ref<DiscoverItem[]>([])
const scanRunning = ref(false)
const scanCompleted = ref(0)
const scanTotal = ref(0)
const scanCurrent = ref('')
const scanDuplicates = ref(0)
const scanAddedCount = ref(0)

const checkedCount = computed(() => discovered.value.filter(i => i.checked).length)
const scanSelectAll = computed({
  get: () => discovered.value.length > 0 && discovered.value.every(i => i.checked),
  set: (val: boolean) => {
    discovered.value.forEach(i => { i.checked = val })
  },
})
const scanPercent = computed(() => scanTotal.value > 0 ? Math.round((scanCompleted.value / scanTotal.value) * 100) : 0)

async function startScan(dir: string) {
  scanDir.value = dir
  scanPhase.value = 'discover'
  scanFinished.value = false
  scanDiscovering.value = true
  discovered.value = []
  scanVisible.value = true
  try {
    const resp = await ebookStore.discoverDirectory(dir)
    discovered.value = resp.files.map(f => ({
      path: f.path,
      title: f.title,
      format: f.format,
      file_size: f.file_size,
      checked: true,
    }))
  } catch {
    discovered.value = []
  } finally {
    scanDiscovering.value = false
    scanFinished.value = true
  }
}

async function pollScan(batchId: string) {
  const deadline = Date.now() + 120000
  for (;;) {
    const p = await ebookStore.getScanProgress(batchId)
    scanCompleted.value = p.completed
    scanTotal.value = p.total
    scanCurrent.value = p.current_book ?? ''
    if (!p.is_running) return
    if (Date.now() > deadline) return
    await new Promise(r => setTimeout(r, 500))
  }
}

async function handleImport() {
  const selected = discovered.value.filter(i => i.checked)
  scanPhase.value = 'import'
  scanRunning.value = true
  scanCompleted.value = 0
  scanTotal.value = selected.length
  scanCurrent.value = ''
  scanDuplicates.value = 0
  scanAddedCount.value = 0
  try {
    const resp = await ebookStore.addBooks(selected.map(i => ({ path: i.path })))
    scanAddedCount.value = resp.added.length
    scanDuplicates.value = resp.duplicates.length
    if (resp.batch_id) {
      await pollScan(resp.batch_id)
    }
    scanRunning.value = false
    await ebookStore.loadShelf()
  } catch {
    scanRunning.value = false
  }
}

const downloadVisible = ref(false)
const mobileDialogWidth = (desktopWidth: string) =>
  computed(() => (isMobileLayout.value ? '92%' : desktopWidth))
const pickerDialogWidth = mobileDialogWidth('540px')
const scanDialogWidth = mobileDialogWidth('640px')
const downloadDialogWidth = mobileDialogWidth('420px')
const moveDialogWidth = mobileDialogWidth('380px')
const exceptionDialogWidth = mobileDialogWidth('480px')
const downloadBook = ref<ShelfBook | null>(null)
const downloadLoaded = ref(0)
const downloadTotal = ref(0)
let downloadAbort: AbortController | null = null

const downloadPercent = computed(() => {
  if (downloadTotal.value <= 0) return 0
  return Math.min(100, Math.round((downloadLoaded.value / downloadTotal.value) * 100))
})

async function startDownload(book: ShelfBook) {
  downloadBook.value = book
  downloadLoaded.value = 0
  downloadTotal.value = book.file_size
  downloadVisible.value = true
  downloadAbort = new AbortController()
  try {
    const data = await downloadWithProgress(book.id, (loaded, total) => {
      downloadLoaded.value = loaded
      if (total > 0) downloadTotal.value = total
    }, downloadAbort.signal)
    await cacheBookData(book.id, book.sha256, data)
    ebookStore.setCached(book.id, true)
    downloadVisible.value = false
    ElMessage.success({ __key: 'ebook.downloadDone' })
    router.push(`/ebooks/read/${book.id}`)
  } catch {
    downloadVisible.value = false
    if (!downloadAbort?.signal.aborted) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
    }
  } finally {
    downloadAbort = null
  }
}

const cancelDownload = () => {
  downloadAbort?.abort()
  downloadVisible.value = false
}

const exceptionVisible = ref(false)
const exceptionState = ref<{ sourceStatus: SourceStatus; title: string; sourcePath: string; bookId: string } | null>(null)

async function handleOpenBook(book: ShelfBook) {
  if (book.scan_status === 'pending' || book.scan_status === 'scanning') {
    ElMessage.info({ __key: 'ebook.scanningMeta' })
    return
  }
  try {
    const meta = await ebookStore.getBookMeta(book.id)
    if (meta.source_status === 'file_not_found' || meta.source_status === 'content_changed') {
      exceptionState.value = {
        sourceStatus: meta.source_status,
        title: book.title,
        sourcePath: meta.book.source_path,
        bookId: book.id,
      }
      exceptionVisible.value = true
      return
    }
    const cached = await getCachedBookData(book.id, book.sha256)
    if (cached) {
      router.push(`/ebooks/read/${book.id}`)
      return
    }
    startDownload(book)
  } catch {
    // error already reported
  }
}

const handleUpdateMeta = async () => {
  const st = exceptionState.value
  if (!st) return
  exceptionVisible.value = false
  try {
    const resp = await ebookStore.relinkBook(st.bookId)
    if (resp.batch_id) await ebookStore.waitForScan(resp.batch_id)
    await ebookStore.loadShelf()
    ElMessage.success({ __key: 'ebook.metaUpdating' })
  } catch {
    // error already reported
  }
}

const handleRelink = () => {
  const st = exceptionState.value
  if (!st) return
  exceptionVisible.value = false
  openPicker('file', async (path) => {
    try {
      const resp = await ebookStore.relinkBook(st.bookId, path)
      if (resp.batch_id) await ebookStore.waitForScan(resp.batch_id)
      await ebookStore.loadShelf()
      ElMessage.success({ __key: 'ebook.relinked' })
    } catch {
      // error already reported
    }
  })
}

const handleExceptionDelete = async () => {
  const st = exceptionState.value
  if (!st) return
  exceptionVisible.value = false
  try {
    await ebookStore.removeBook(st.bookId)
    ElMessage.success({ __key: 'ebook.removedWithCache' })
  } catch {
    // error already reported
  }
}

const handleRemoveBook = (book: ShelfBook) => {
  ElMessageBox.confirm(
    t('ebook.removeConfirm', { title: book.title }),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
    }
  ).then(async () => {
    try {
      await ebookStore.removeBook(book.id)
      ElMessage.success({ __key: 'ebook.removedWithCache' })
    } catch {
      // error already reported
    }
  }).catch(() => {})
}

const moveVisible = ref(false)
const moveTarget = ref<'root' | string>('root')
const moveBookId = ref<string | null>(null)

const openMoveDialog = (book: ShelfBook) => {
  moveBookId.value = book.id
  moveTarget.value = book.category_id === null ? 'root' : book.category_id
  moveVisible.value = true
}

const handleMoveConfirm = async () => {
  if (moveBookId.value) {
    try {
      await ebookStore.moveBook(moveBookId.value, moveTarget.value === 'root' ? null : moveTarget.value)
      ElMessage.success({ __key: 'ebook.moved' })
    } catch {
      // error already reported
    }
  }
  moveVisible.value = false
}

onMounted(async () => {
  checkLayout()
  window.addEventListener('resize', checkLayout)
  if (!userStore.user?.ebook_enabled) return
  await ebookStore.loadShelf()
  await Promise.all(ebookStore.books.map(async (b) => {
    try {
      if (await isBookCached(b.id, b.sha256)) ebookStore.setCached(b.id, true)
    } catch {
      // ignore cache check failure
    }
  }))
})

onUnmounted(() => {
  window.removeEventListener('resize', checkLayout)
  downloadAbort?.abort()
  previewUrls.forEach(url => URL.revokeObjectURL(url))
  previewUrls.clear()
})
</script>

<style scoped>
.ebooks-container {
  height: 100%;
  display: flex;
  flex: 1;
  min-height: 0;
  box-sizing: border-box;
  padding: 20px;
  gap: 16px;
}

/* ===== Category Panel ===== */
.category-panel {
  width: 200px;
  min-width: 200px;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 12px;
  background: var(--el-bg-color);
  overflow: hidden;
}

.category-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.category-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 8px;
  cursor: pointer;
  color: var(--el-text-color-regular);
  transition: all 0.2s ease;
  margin-bottom: 2px;
}

.category-item:hover {
  background: var(--el-fill-color-light);
}

.category-item.active {
  background: rgba(14, 165, 233, 0.12);
  color: var(--el-color-primary);
  font-weight: 600;
}

.category-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
}

.category-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color);
  border-radius: 10px;
  padding: 1px 8px;
  flex-shrink: 0;
}

.category-more {
  display: flex;
  align-items: center;
  color: var(--el-text-color-secondary);
  padding: 2px;
}

.category-item > .el-dropdown {
  display: none;
}

.category-item:hover > .el-dropdown {
  display: flex;
  align-items: center;
}

.category-footer {
  padding: 8px;
  border-top: 1px solid var(--el-border-color-lighter);
}

/* ===== Shelf Panel ===== */
.shelf-panel {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.shelf-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.toolbar-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.book-count {
  font-size: 13px;
  font-weight: 400;
  color: var(--el-text-color-secondary);
}

.btn-icon {
  margin-right: 4px;
}

.shelf-grid-wrapper {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.shelf-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  gap: 16px;
  padding-bottom: 16px;
}

/* ===== Book Card ===== */
.book-card {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 12px;
  overflow: hidden;
  cursor: pointer;
  background: var(--el-bg-color);
  transition: all 0.25s ease;
  display: flex;
  flex-direction: column;
}

.book-card:hover {
  transform: translateY(-3px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
}

.book-cover {
  position: relative;
  aspect-ratio: 3 / 4;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  box-sizing: border-box;
}

.cover-image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover-title {
  position: relative;
  color: rgba(255, 255, 255, 0.95);
  font-size: 16px;
  font-weight: 700;
  text-align: center;
  line-height: 1.5;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.format-badge {
  position: absolute;
  top: 8px;
  left: 8px;
  font-size: 10px;
  font-weight: 700;
  color: #ffffff;
  background: rgba(0, 0, 0, 0.35);
  border-radius: 4px;
  padding: 2px 6px;
  letter-spacing: 0.5px;
}

.cached-badge {
  position: absolute;
  bottom: 8px;
  right: 8px;
  color: #67c23a;
  background: rgba(255, 255, 255, 0.9);
  border-radius: 50%;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
}

.cover-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.cover-spinner {
  animation: ebook-spin 1s linear infinite;
  color: #ffffff;
  font-size: 28px;
}

.overlay-text {
  color: #ffffff;
  font-size: 12px;
}

.book-info {
  padding: 10px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.book-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-meta {
  display: flex;
  gap: 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.book-added {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.book-progress {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-height: 1.4em;
}

.book-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  margin-top: 2px;
}

.book-actions .el-button {
  margin: 0;
  flex: 1;
  padding-left: 8px;
  padding-right: 8px;
}

/* ===== Picker Dialog ===== */
.picker-breadcrumb {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 2px;
  margin-bottom: 10px;
  font-size: 13px;
}

.breadcrumb-sep {
  color: var(--el-text-color-secondary);
  margin: 0 2px;
}

.picker-list {
  height: 320px;
  overflow-y: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 4px;
}

.picker-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.picker-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.15s ease;
}

.picker-entry:hover {
  background: var(--el-fill-color-light);
}

.picker-entry.disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.picker-icon {
  font-size: 18px;
  color: var(--el-color-primary);
  flex-shrink: 0;
}

.picker-icon.is-folder {
  color: #e6a23c;
}

.picker-name {
  flex: 1;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--el-text-color-primary);
}

.picker-size {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}

/* ===== Scan Dialog ===== */
.scan-status {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  font-size: 13px;
  color: var(--el-text-color-regular);
}

.scan-spinner {
  animation: ebook-spin 1s linear infinite;
  color: var(--el-color-primary);
}

@keyframes ebook-spin {
  to {
    transform: rotate(360deg);
  }
}

.scan-current {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scan-dup {
  font-size: 12px;
  color: var(--el-color-warning);
  margin-top: 10px;
}

.scan-list {
  max-height: 340px;
  overflow-y: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 4px;
}

.scan-entry {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
}

.scan-entry:hover {
  background: var(--el-fill-color-light);
}

.scan-format {
  font-size: 10px;
  font-weight: 700;
  color: var(--el-color-primary);
  background: rgba(14, 165, 233, 0.12);
  border-radius: 4px;
  padding: 2px 6px;
  flex-shrink: 0;
  letter-spacing: 0.5px;
}

.scan-name {
  font-size: 13px;
  color: var(--el-text-color-primary);
  flex-shrink: 0;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scan-path {
  flex: 1;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scan-size {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}

.scan-footer-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-top: 10px;
}

.scan-selected {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

/* ===== Download Dialog ===== */
.download-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.download-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.download-bytes {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  text-align: right;
}

/* ===== Move Dialog ===== */
.move-radio-group {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;
  max-height: 280px;
  overflow-y: auto;
  width: 100%;
}

/* ===== Exception Dialog ===== */
.exception-desc {
  font-size: 14px;
  line-height: 1.7;
  color: var(--el-text-color-regular);
  margin: 0;
  word-break: break-all;
}

/* ===== Mobile ===== */
.ebooks-container.is-mobile {
  flex-direction: column;
  padding: 12px 8px;
  gap: 10px;
}

.ebooks-container.is-mobile .category-panel {
  width: 100%;
  min-width: 0;
  flex-direction: row;
  align-items: center;
  border-radius: 10px;
}

.ebooks-container.is-mobile .category-list {
  display: flex;
  flex-direction: row;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 6px;
  gap: 4px;
}

.ebooks-container.is-mobile .category-item {
  flex-shrink: 0;
  margin-bottom: 0;
  padding: 6px 10px;
}

.ebooks-container.is-mobile .category-item > .el-dropdown {
  display: flex;
  align-items: center;
}

.ebooks-container.is-mobile .category-footer {
  border-top: none;
  border-left: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
  padding: 4px;
}

.ebooks-container.is-mobile .shelf-grid {
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 10px;
}
</style>

<style>
/* 移动端 Message box 适配（message box 渲染在 body 下，需全局样式） */
@media (max-width: 767px) {
  .el-message-box {
    width: 92% !important;
    max-width: 92% !important;
  }
}
</style>
