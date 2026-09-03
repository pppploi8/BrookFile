<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Loading } from '@element-plus/icons-vue'
import ReaderShell from '@/components/reader/ReaderShell.vue'
import { ebookBookMeta, ebookBookDownload, type BookMetaResponse } from '@/api/ebook'
import { getCachedBookData, cacheBookData, downloadWithProgress } from '@/utils/ebookCache'
import type { BookFormat } from '@/reader/types'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()

const bookId = computed(() => String(route.params.id))
const meta = ref<BookMetaResponse | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)
const downloadProgress = ref(0)
const downloading = ref(false)
const abortController = ref<AbortController | null>(null)

const title = computed(() => meta.value?.book.title ?? '')
const format = computed<BookFormat>(() => (meta.value?.book.format ?? 'pdf') as BookFormat)

let cachedData: ArrayBuffer | null = null

async function loadBook() {
  loading.value = true
  error.value = null
  try {
    const resp = await ebookBookMeta(bookId.value)
    if (!resp.success) {
      error.value = resp.fail_code || 'LOAD_FAILED'
      loading.value = false
      return
    }
    meta.value = resp

    if (resp.source_status !== 'ok') {
      error.value = resp.source_status
      loading.value = false
      return
    }

    const cached = await getCachedBookData(bookId.value, resp.book.sha256)
    if (cached) {
      cachedData = cached
      loading.value = false
      return
    }

    downloading.value = true
    abortController.value = new AbortController()
    try {
      const data = await downloadWithProgress(
        bookId.value,
        (loaded, total) => {
          if (total > 0) downloadProgress.value = Math.round((loaded / total) * 100)
        },
        abortController.value.signal,
      )
      await cacheBookData(bookId.value, resp.book.sha256, data)
      cachedData = data
    } catch (e) {
      if (e instanceof Error && e.name === 'AbortError') {
        goBack()
        return
      }
      error.value = 'DOWNLOAD_FAILED'
    } finally {
      downloading.value = false
      abortController.value = null
    }
    loading.value = false
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'LOAD_FAILED'
    loading.value = false
  }
}

const getSource = async (): Promise<ArrayBuffer> => {
  if (cachedData) return cachedData
  const data = await ebookBookDownload(bookId.value)
  cachedData = data
  return data
}

const chapters = computed(() => meta.value?.chapters ?? [])

function goBack(): void {
  if (window.history.length > 1) router.back()
  else router.push('/ebooks')
}

onMounted(() => {
  loadBook()
})

onUnmounted(() => {
  if (abortController.value) abortController.value.abort()
})
</script>

<template>
  <div v-if="loading && !downloading" class="reader-unsupported">
    <el-icon class="is-loading" :size="32"><Loading /></el-icon>
    <span class="loading-text">{{ t('reader.loading') }}</span>
  </div>
  <div v-else-if="downloading" class="reader-unsupported">
    <div class="download-progress-container">
      <div class="download-title">{{ title }}</div>
      <el-progress :percentage="downloadProgress" :stroke-width="10" />
      <el-button class="download-cancel" @click="abortController?.abort()">{{ t('common.cancel') }}</el-button>
    </div>
  </div>
  <div v-else-if="error" class="reader-unsupported">
    <el-result
      v-if="error === 'file_not_found'"
      icon="warning"
      :title="t('ebook.srcMissingTitle')"
      :sub-title="t('ebook.srcMissingDesc', { title, path: meta?.book.source_path ?? '' })"
    >
      <template #extra>
        <el-button @click="goBack">{{ t('common.back') }}</el-button>
      </template>
    </el-result>
    <el-result
      v-else-if="error === 'content_changed'"
      icon="warning"
      :title="t('ebook.srcChangedTitle')"
      :sub-title="t('ebook.srcChangedDesc', { title })"
    >
      <template #extra>
        <el-button @click="goBack">{{ t('common.back') }}</el-button>
      </template>
    </el-result>
    <el-result v-else icon="error" :title="t('reader.loadError')" :sub-title="error">
      <template #extra>
        <el-button @click="goBack">{{ t('common.back') }}</el-button>
      </template>
    </el-result>
  </div>
  <ReaderShell
    v-else-if="meta"
    :book-id="bookId"
    :title="title"
    :format="format"
    :chapters="chapters"
    :get-source="getSource"
    @back="goBack"
  />
</template>

<style scoped>
.reader-unsupported {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
}
.loading-text {
  font-size: 14px;
  color: var(--el-text-color-secondary);
}
.download-progress-container {
  width: 420px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  align-items: center;
}
.download-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  text-align: center;
  word-break: break-all;
}
.download-progress-container .el-progress {
  width: 100%;
}
.download-cancel {
  margin-top: 8px;
}
</style>
