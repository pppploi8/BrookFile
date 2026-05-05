<template>
  <div class="files-container" :class="{ 'is-mobile': isMobileLayout }">
    <div class="files-toolbar">
      <div class="breadcrumb-wrapper">
        <el-breadcrumb separator="/" class="path-breadcrumb">
          <el-breadcrumb-item>
            <span class="breadcrumb-item-link" @click="handlePathClick(-1)">
              <el-icon><HomeFilled /></el-icon>
              {{ t('files.rootDirectory') }}
            </span>
          </el-breadcrumb-item>
          <el-breadcrumb-item
            v-for="(item, index) in pathList"
            :key="index"
          >
            <span
              class="breadcrumb-item-link"
              @click="handlePathClick(index)"
            >
              {{ item }}
            </span>
          </el-breadcrumb-item>
        </el-breadcrumb>
      </div>

      <div class="toolbar-actions">
        <input
          ref="fileInputRef"
          type="file"
          multiple
          style="display: none"
          @change="handleFileSelect"
        />
        <input
          ref="folderInputRef"
          type="file"
          webkitdirectory
          style="display: none"
          @change="handleFolderSelect"
        />
        <el-button type="primary" @click="openFileSelector">
          <el-icon><Upload /></el-icon>
          <span class="btn-text">{{ t('files.uploadFile') }}</span>
        </el-button>

        <el-button type="primary" @click="openFolderSelector">
          <el-icon><Upload /></el-icon>
          <span class="btn-text">{{ t('files.uploadFolderShort') }}</span>
        </el-button>

        <el-button @click="handleNewFolder">
          <el-icon><FolderAdd /></el-icon>
          <span class="btn-text">{{ t('files.newFolderShort') }}</span>
        </el-button>

        <el-input
          v-if="!isMobileLayout"
          v-model="searchKeyword"
          class="search-input"
          :placeholder="t('files.searchPlaceholder')"
          :prefix-icon="Search"
          clearable
          style="margin-left: 12px"
        />
      </div>
    </div>

    <div class="table-wrapper">
      <el-table
        :data="filteredFileList"
        style="width: 100%"
        stripe
        border
        height="100%"
        show-overflow-tooltip
        v-loading="loading"
        @selection-change="handleSelectionChange"
      >
      <el-table-column type="selection" width="38" :selectable="(row: FileItem) => row.file_type !== 'parent'" />
      <el-table-column :label="t('files.fileName')" min-width="300" prop="name" sortable :sort-method="sortByName">
        <template #default="{ row }">
          <div
            class="file-name-cell"
            :class="{ clickable: row.file_type === 'directory' || row.file_type === 'parent' }"
            @click="handleFileClick(row)"
          >
            <el-icon class="file-icon" :class="row.file_type === 'directory' ? 'is-folder' : 'is-parent'">
              <Folder v-if="row.file_type === 'directory'" />
              <Back v-else-if="row.file_type === 'parent'" />
              <Document v-else />
            </el-icon>
            <span class="file-name">{{ row.file_type === 'parent' ? t('files.parentDirectory') : row.name }}</span>
          </div>
        </template>
      </el-table-column>

      <el-table-column :label="t('files.fileSize')" width="150" prop="size" sortable :sort-method="sortBySize">
        <template #default="{ row }">
          {{ row.file_type === 'directory' || row.file_type === 'parent' ? '-' : formatFileSize(row.size) }}
        </template>
      </el-table-column>

      <el-table-column :label="t('files.modifiedTime')" width="200" prop="modified" sortable :sort-method="sortByModified">
        <template #default="{ row }">
          {{ row.modified ? formatUtcTimestamp(row.modified) : '-' }}
        </template>
      </el-table-column>

      <el-table-column :label="t('files.operations')" width="200" fixed="right">
        <template #default="{ row }">
          <template v-if="row.file_type === 'parent'">
          </template>
          <template v-else-if="row.file_type === 'directory'">
            <el-link type="primary" :icon="Share" @click="handleShare(row)">{{ t('share.shareAction') }}</el-link>
            <el-link type="primary" :icon="Download" @click="handleFolderDownload(row)">{{ t('files.download') }}</el-link>
            <el-link type="danger" :icon="Delete" @click="handleDelete(row)">{{ t('files.delete') }}</el-link>
          </template>
          <template v-else>
            <el-link type="primary" :icon="Share" @click="handleShare(row)">{{ t('share.shareAction') }}</el-link>
            <el-link type="primary" :icon="Download" @click="handleDownload(row)">{{ t('files.download') }}</el-link>
            <el-link type="danger" :icon="Delete" @click="handleDelete(row)">{{ t('files.delete') }}</el-link>
          </template>
        </template>
      </el-table-column>
    </el-table>
    
    <transition name="slide-up">
      <div v-if="selectedFiles.length > 0" class="selection-bar">
        <span class="selection-count">{{ t('files.selectedCount', { count: selectedFiles.length }) }}</span>
        <div class="selection-actions">
          <el-button type="primary" :icon="FolderRemove" @click="openMoveDialog">{{ t('files.move') }}</el-button>
          <el-button type="danger" :icon="Delete" @click="handleBatchDelete">{{ t('files.delete') }}</el-button>
        </div>
      </div>
    </transition>
    </div>

    <el-dialog
      v-model="moveDialogVisible"
      :title="t('files.moveFiles')"
      width="min(500px, calc(100vw - 32px))"
    >
      <div class="move-dialog-content">
        <div class="move-info">
          <span>{{ t('files.targetFolder') }}:</span>
          <span class="move-path">{{ moveTargetPath || t('files.rootDirectory') }}</span>
        </div>
        <div class="move-browser">
          <div class="move-browser-header">
            <el-button 
              size="small" 
              :disabled="!moveBrowserHasParent"
              @click="moveBrowserGoParent"
            >
              {{ t('init.parentFolder') }}
            </el-button>
          </div>
          <div class="move-browser-list" v-loading="moveBrowserLoading">
            <div 
              v-for="folder in moveBrowserFolders" 
              :key="folder.name"
              class="move-folder-item"
              @click="handleMoveFolderClick(folder)"
            >
              <el-icon><Folder /></el-icon>
              <span>{{ folder.name }}</span>
            </div>
            <div v-if="moveBrowserFolders.length === 0" class="move-empty">
              {{ t('files.noFoldersAvailable') }}
            </div>
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="moveDialogVisible = false">
          {{ t('common.cancel') }}
        </el-button>
        <el-button type="primary" @click="confirmMove">
          {{ t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="newFolderDialogVisible"
      :title="t('files.createFolder')"
      width="min(400px, calc(100vw - 32px))"
    >
      <el-input
        v-model="newFolderName"
        :placeholder="t('files.folderNamePlaceholder')"
      />
      <template #footer>
        <el-button @click="newFolderDialogVisible = false">
          {{ t('common.cancel') }}
        </el-button>
        <el-button type="primary" @click="confirmCreateFolder">
          {{ t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="uploadDialogVisible"
      :title="t('files.uploadProgress')"
      width="min(600px, calc(100vw - 32px))"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
    >
      <div class="upload-list" v-if="uploadTasks.length > 0">
        <div
          v-for="task in sortedUploadTasks"
          :key="task.id"
          class="upload-item"
        >
          <div class="upload-item-header">
            <span class="upload-item-name" :title="task.relativePath">{{ task.relativePath }}</span>
            <el-tag
              :type="getStatusTagType(task.status)"
              size="small"
            >
              {{ getTaskStatusText(task) }}
            </el-tag>
          </div>
          <div class="upload-item-info">
            <span class="upload-item-size">{{ formatFileSize(task.size) }}</span>
            <span class="upload-item-progress" v-if="task.status === 'uploading'">
              {{ formatFileSize(task.uploadedBytes) }} / {{ formatFileSize(task.size) }} ({{ task.progress.toFixed(2) }}%)
            </span>
          </div>
          <el-progress
            :percentage="task.progress"
            :status="task.status === 'completed' ? 'success' : task.status === 'failed' ? 'exception' : undefined"
            :stroke-width="6"
            :show-text="false"
          />
        </div>
      </div>
      <div class="upload-empty" v-else>
        {{ t('files.noFiles') }}
      </div>
      <template #footer>
        <div class="upload-dialog-footer">
          <span class="upload-summary">
            {{ t('files.uploading') }}: {{ uploadingCount }} / {{ t('files.waiting') }}: {{ waitingCount }} / {{ t('files.completed') }}: {{ completedCount }} / {{ t('files.cancelled') }}: {{ cancelledCount }}
          </span>
          <div class="upload-actions">
            <el-button v-if="hasCancellableTasks" @click="cancelAllUploads">
              {{ t('files.cancelAll') }}
            </el-button>
            <el-button type="primary" @click="closeUploadDialog">
              {{ t('files.close') }}
            </el-button>
          </div>
        </div>
      </template>
    </el-dialog>

    <el-dialog
      v-model="downloadDialogVisible"
      :title="t('files.downloadProgress')"
      width="min(500px, calc(100vw - 32px))"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
    >
      <div class="download-item" v-if="downloadTask">
        <div class="upload-item-header">
          <span class="upload-item-name" :title="downloadTask.name">{{ downloadTask.name }}</span>
          <el-tag
            :type="downloadTask.status === 'completed' ? 'success' : downloadTask.status === 'failed' ? 'danger' : 'primary'"
            size="small"
          >
            {{ downloadTask.status === 'downloading' ? t('files.downloading') : downloadTask.status === 'completed' ? t('files.downloadCompleted') : t('files.downloadFailed') }}
          </el-tag>
        </div>
        <div class="upload-item-info">
          <span class="upload-item-size">{{ formatFileSize(downloadTask.size) }}</span>
          <span class="upload-item-progress" v-if="downloadTask.status === 'downloading'">
            {{ formatFileSize(downloadTask.downloadedBytes) }} / {{ formatFileSize(downloadTask.size) }} ({{ downloadTask.progress.toFixed(2) }}%)
          </span>
        </div>
        <el-progress
          :percentage="downloadTask.progress"
          :status="downloadTask.status === 'completed' ? 'success' : downloadTask.status === 'failed' ? 'exception' : undefined"
          :stroke-width="6"
          :show-text="false"
        />
      </div>
      <template #footer>
        <div class="upload-dialog-footer">
          <span></span>
          <div class="upload-actions">
            <el-button v-if="downloadTask?.status === 'completed'" type="primary" @click="saveDownloadedFile">
              {{ t('files.saveFile') }}
            </el-button>
            <el-button v-if="downloadTask?.status === 'failed'" type="primary" @click="redownloadFile">
              {{ t('files.saveFile') }}
            </el-button>
            <el-button @click="closeDownloadDialog">
              {{ t('files.close') }}
            </el-button>
          </div>
        </div>
      </template>
    </el-dialog>

    <el-dialog
      v-model="folderDownloadDialogVisible"
      :title="t('files.folderDownloadProgress')"
      width="min(500px, calc(100vw - 32px))"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
    >
      <div class="download-item" v-if="folderDownloadTask">
        <div class="upload-item-header">
          <span class="upload-item-name" :title="folderDownloadTask.name">{{ folderDownloadTask.name }}</span>
          <el-tag
            :type="getFolderDownloadTagType()"
            size="small"
          >
            {{ getFolderDownloadStatusText() }}
          </el-tag>
        </div>
        <template v-if="folderDownloadTask.size > 0">
          <div class="upload-item-info">
            <span class="upload-item-size">{{ formatFileSize(folderDownloadTask.size) }}</span>
            <span class="upload-item-progress" v-if="folderDownloadTask.downloadStatus === 'downloading'">
              {{ formatFileSize(folderDownloadTask.downloadedBytes) }} / {{ formatFileSize(folderDownloadTask.size) }} ({{ folderDownloadTask.progress.toFixed(2) }}%)
              <template v-if="folderDownloadTask.speed > 0">&nbsp;{{ formatDownloadSpeed(folderDownloadTask.speed) }}</template>
            </span>
          </div>
          <el-progress
            :percentage="folderDownloadTask.progress"
            :status="folderDownloadTask.downloadStatus === 'completed' ? 'success' : folderDownloadTask.downloadStatus === 'failed' ? 'exception' : undefined"
            :stroke-width="6"
            :show-text="false"
          />
        </template>
        <template v-else>
          <div class="upload-item-info" style="margin-top: 8px;">
            <span class="upload-item-progress">
              {{ formatFileSize(folderDownloadTask.downloadedBytes) }}
              <template v-if="folderDownloadTask.downloadStatus === 'downloading' && folderDownloadTask.speed > 0">
                &nbsp;({{ formatDownloadSpeed(folderDownloadTask.speed) }})
              </template>
            </span>
          </div>
        </template>
      </div>
      <template #footer>
        <div class="upload-dialog-footer">
          <span></span>
          <div class="upload-actions">
            <el-button v-if="folderDownloadTask?.downloadStatus === 'completed'" type="primary" @click="saveFolderDownloadedFile">
              {{ t('files.saveFile') }}
            </el-button>
            <el-button v-if="folderDownloadTask?.downloadStatus === 'failed'" type="primary" @click="retryFolderDownload">
              {{ t('files.redownload') }}
            </el-button>
            <el-button @click="closeFolderDownloadDialog">
              {{ t('files.close') }}
            </el-button>
          </div>
        </div>
      </template>
    </el-dialog>

    <el-drawer
      v-model="shareDrawerVisible"
      :title="t('share.shareAction')"
      direction="rtl"
      size="min(380px, calc(100vw - 32px))"
    >
      <div v-if="shareDrawerLoading" class="share-drawer-loading">
        <el-icon class="is-loading" :size="24"><Loading /></el-icon>
      </div>

      <div v-else-if="existingShare" class="share-info-card">
        <div class="share-info-row">
          <span class="share-info-label">{{ t('share.shareLink') }}</span>
          <span class="share-info-value share-link-text">
            {{ existingShare.share_mode === 'direct' ? `${windowLocationOrigin}/api/share/file/${existingShare.share_code}` : `${windowLocationOrigin}/s/${existingShare.share_code}` }}
          </span>
          <el-button type="primary" size="small" @click="copyShareLink" class="share-copy-btn">
            {{ t('share.copyLink') }}
          </el-button>
        </div>
        <el-divider />
        <div class="share-info-row">
          <span class="share-info-label">{{ t('share.shareMode') }}</span>
          <span class="share-info-value">{{ existingShare.share_mode === 'direct' ? t('share.directLink') : t('share.downloadPage') }}</span>
        </div>
        <div class="share-info-row">
          <span class="share-info-label">{{ t('share.expireType') }}</span>
          <span class="share-info-value">
            <template v-if="existingShare.expire_type === 'permanent'">{{ t('share.permanent') }}</template>
            <template v-else-if="existingShare.expire_type === 'time'">{{ formatUtcDatetimeString(existingShare.expire_at!) }}</template>
            <template v-else-if="existingShare.expire_type === 'count'">{{ existingShare.max_downloads }} {{ t('share.times') }}</template>
          </span>
        </div>
        <div class="share-info-row">
          <span class="share-info-label">{{ t('share.hasPassword') }}</span>
          <span class="share-info-value">{{ existingShare.has_password ? t('share.yes') : t('share.no') }}</span>
        </div>
        <div class="share-info-row">
          <span class="share-info-label">{{ t('share.downloadCount') }}</span>
          <span class="share-info-value">{{ existingShare.download_count }}</span>
        </div>
        <el-divider />
        <el-button type="danger" @click="handleCancelShare">
          {{ t('share.cancelShare') }}
        </el-button>
      </div>

      <div v-else class="share-form">
        <div class="share-form-row">
          <span class="share-info-label">{{ t('share.fileName') }}</span>
          <span class="share-info-value">{{ shareFileName }}</span>
        </div>

        <div class="share-form-section">
          <div class="share-form-label">{{ t('share.expireType') }}</div>
          <el-radio-group v-model="shareForm.expire_type">
            <el-radio value="permanent">{{ t('share.permanent') }}</el-radio>
            <el-radio value="time">{{ t('share.expireByTime') }}</el-radio>
            <el-radio value="count">{{ t('share.expireByCount') }}</el-radio>
          </el-radio-group>
          <el-date-picker
            v-if="shareForm.expire_type === 'time'"
            v-model="shareForm.expire_at"
            type="datetime"
            :placeholder="t('share.expireAt')"
            style="margin-top: 8px; width: 100%"
          />
          <el-input-number
            v-if="shareForm.expire_type === 'count'"
            v-model="shareForm.max_downloads"
            :min="1"
            style="margin-top: 8px; width: 100%"
          />
        </div>

        <div class="share-form-section">
          <div class="share-form-label">{{ t('share.shareMode') }}</div>
          <el-radio-group v-model="shareForm.share_mode" :disabled="shareForm.usePassword">
            <el-radio value="page">{{ t('share.downloadPage') }}</el-radio>
            <el-radio value="direct" :disabled="shareForm.usePassword">{{ t('share.directLink') }}</el-radio>
          </el-radio-group>
          <div v-if="shareForm.usePassword" class="share-form-hint">{{ t('share.directLinkPasswordConflict') }}</div>
        </div>

        <div class="share-form-section">
          <el-checkbox v-model="shareForm.usePassword" :disabled="shareForm.share_mode === 'direct'">{{ t('share.setPassword') }}</el-checkbox>
          <el-input
            v-if="shareForm.usePassword"
            v-model="shareForm.password"
            type="password"
            show-password
            :placeholder="t('share.passwordPlaceholder')"
            style="margin-top: 8px"
          />
          <div v-if="shareForm.share_mode === 'direct'" class="share-form-hint">{{ t('share.directLinkNoPassword') }}</div>
        </div>

        <el-button type="primary" style="width: 100%; margin-top: 16px" @click="handleCreateShare" :loading="shareCreating">
          {{ t('share.createShare') }}
        </el-button>
      </div>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox } from 'element-plus'
import { Upload, FolderAdd, Search, Delete, Folder, Document, HomeFilled, Download, FolderRemove, Back, Loading, Share } from '@element-plus/icons-vue'
import { formatUtcTimestamp, formatUtcDatetimeString } from '@/utils/date'
import { formatFileSize, formatDownloadSpeed } from '@/utils/format'
import { browseFiles, createFolder, deleteFile, uploadStart, uploadChunk, uploadComplete, uploadCancel, fetchDownloadFolder, fetchDownloadFile, moveFiles, batchDeleteFiles } from '@/api'
import type { FileItem } from '@/api'
import { getShareByPath, createShare, deleteShares, getSystemInfo, type ShareItem } from '@/api/system'
import { useUserStore } from '@/stores/user'

interface UploadTask {
  id: string
  uploadId?: string
  file: File
  relativePath: string
  size: number
  progress: number
  uploadedBytes: number
  status: 'waiting' | 'uploading' | 'completed' | 'failed' | 'cancelled'
  failCode?: string
  abortController?: AbortController
}

interface DownloadTask {
  name: string
  path: string
  size: number
  progress: number
  downloadedBytes: number
  status: 'downloading' | 'completed' | 'failed'
  blob?: Blob
  abortController?: AbortController
}

interface FolderDownloadTask {
  name: string
  path: string
  downloadStatus: 'downloading' | 'completed' | 'failed'
  size: number
  progress: number
  downloadedBytes: number
  speed: number
  blob?: Blob
  abortController?: AbortController
}

const { t } = useI18n()
const userStore = useUserStore()

// 移动端检测
const isMobileLayout = ref(false)

const checkLayout = () => {
  const width = window.innerWidth
  const height = window.innerHeight
  const aspectRatio = height / width
  isMobileLayout.value = aspectRatio > 1.2 || width < 768
}

onMounted(() => {
  checkLayout()
  window.addEventListener('resize', checkLayout)
  loadFiles()
})

onUnmounted(() => {
  window.removeEventListener('resize', checkLayout)
  if (folderDownloadTask.value?.abortController && folderDownloadTask.value.downloadStatus === 'downloading') {
    folderDownloadTask.value.abortController.abort()
  }
  if (downloadTask.value?.abortController && downloadTask.value.status === 'downloading') {
    downloadTask.value.abortController.abort()
  }
})

const fileInputRef = ref<HTMLInputElement>()
const folderInputRef = ref<HTMLInputElement>()
const searchKeyword = ref('')
const currentPath = ref('')
const newFolderDialogVisible = ref(false)
const newFolderName = ref('')
const fileList = ref<FileItem[]>([])
const loading = ref(false)
const uploadDialogVisible = ref(false)
const uploadTasks = ref<UploadTask[]>([])
const optimalChunkSize = ref(8 * 1024 * 1024)
const downloadDialogVisible = ref(false)
const downloadTask = ref<DownloadTask | null>(null)
const folderDownloadDialogVisible = ref(false)
const folderDownloadTask = ref<FolderDownloadTask | null>(null)
const selectedFiles = ref<FileItem[]>([])
const moveDialogVisible = ref(false)
const moveTargetPath = ref('')
const moveBrowserPath = ref('')
const moveBrowserFolders = ref<{ name: string; path: string }[]>([])
const moveBrowserHasParent = ref(false)
const moveBrowserLoading = ref(false)

const shareDrawerVisible = ref(false)
const shareDrawerLoading = ref(false)
const existingShare = ref<ShareItem | null>(null)
const shareFilePath = ref('')
const shareFileName = ref('')
const shareCreating = ref(false)
const windowLocationOrigin = window.location.origin
const shareForm = ref({
  expire_type: 'permanent',
  expire_at: null as string | null,
  max_downloads: 10 as number | null,
  share_mode: 'page',
  usePassword: false,
  password: '',
})

const pathList = computed(() => {
  if (!currentPath.value) return []
  return currentPath.value.split('/').filter(Boolean)
})

const filteredFileList = computed(() => {
  let files = fileList.value
  if (searchKeyword.value) {
    files = files.filter(file =>
      file.name.toLowerCase().includes(searchKeyword.value.toLowerCase())
    )
  }
  const sortedFiles = [...files].sort((a, b) => {
    if (a.file_type === 'directory' && b.file_type !== 'directory') return -1
    if (a.file_type !== 'directory' && b.file_type === 'directory') return 1
    return a.name.localeCompare(b.name)
  })

  // 在非根目录时添加"返回上级"虚拟节点
  if (currentPath.value && !searchKeyword.value) {
    const parentItem: FileItem = {
      name: '..',
      file_type: 'parent',
      size: 0,
      modified: ''
    }
    return [parentItem, ...sortedFiles]
  }

  return sortedFiles
})

const uploadingCount = computed(() => uploadTasks.value.filter(t => t.status === 'uploading').length)
const waitingCount = computed(() => uploadTasks.value.filter(t => t.status === 'waiting').length)
const completedCount = computed(() => uploadTasks.value.filter(t => t.status === 'completed').length)
const cancelledCount = computed(() => uploadTasks.value.filter(t => t.status === 'cancelled').length)
const hasCancellableTasks = computed(() => uploadTasks.value.some(t => t.status === 'waiting' || t.status === 'uploading'))
const sortedUploadTasks = computed(() => {
  const statusOrder: Record<string, number> = {
    'uploading': 0,
    'waiting': 1,
    'completed': 2,
    'failed': 3,
    'cancelled': 4
  }
  return [...uploadTasks.value].sort((a, b) => (statusOrder[a.status] ?? 0) - (statusOrder[b.status] ?? 0))
})

const handleSelectionChange = (selection: FileItem[]) => {
  selectedFiles.value = selection
}

const loadMoveBrowserFolders = async () => {
  moveBrowserLoading.value = true
  try {
    const response = await browseFiles(moveBrowserPath.value || undefined)
    const folders = response.files
      .filter(f => f.file_type === 'directory')
      .map(f => ({
        name: f.name,
        path: moveBrowserPath.value ? `${moveBrowserPath.value}/${f.name}` : f.name
      }))
    moveBrowserFolders.value = folders
    moveBrowserHasParent.value = moveBrowserPath.value !== ''
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
    }
  }
  finally {
    moveBrowserLoading.value = false
  }
}

const openMoveDialog = () => {
  moveTargetPath.value = ''
  moveBrowserPath.value = ''
  loadMoveBrowserFolders()
  moveDialogVisible.value = true
}

const handleMoveFolderClick = (folder: { name: string; path: string }) => {
  moveBrowserPath.value = folder.path
  moveTargetPath.value = folder.path
  loadMoveBrowserFolders()
}

const moveBrowserGoParent = () => {
  if (!moveBrowserPath.value) return
  const parts = moveBrowserPath.value.split('/')
  parts.pop()
  moveBrowserPath.value = parts.join('/')
  loadMoveBrowserFolders()
  moveTargetPath.value = moveBrowserPath.value
}

const confirmMove = async () => {
  if (moveTargetPath.value === currentPath.value) {
    ElMessage.error({ __key: 'files.cannotMoveToSameFolder' })
    return
  }
  
  const selectedFolderNames = selectedFiles.value
    .filter(f => f.file_type === 'directory')
    .map(f => f.name)
  
  for (const folderName of selectedFolderNames) {
    const folderPath = currentPath.value ? `${currentPath.value}/${folderName}` : folderName
    if (moveTargetPath.value === folderPath || 
        (moveTargetPath.value && moveTargetPath.value.startsWith(folderPath + '/'))) {
      ElMessage.error({ __key: 'files.cannotMoveFolderIntoItself' })
      return
    }
  }
  
  const fileNames = selectedFiles.value.map(f => f.name)
  
  try {
    const response = await moveFiles(fileNames, currentPath.value || undefined, moveTargetPath.value || undefined)
    
    if (response.success) {
      ElMessage.success({ __key: 'files.moveSuccess' })
      moveDialogVisible.value = false
      selectedFiles.value = []
      loadFiles()
    } else if (response.fail_code === 'FILES_ALREADY_EXIST' && response.conflict_files) {
      ElMessage.error({ __key: 'files.conflictFiles', __params: { files: response.conflict_files.join(', ') } })
    } else if (response.fail_code === 'MOVE_FAILED' && response.failed_files) {
      ElMessage.error({ __key: 'files.moveFailed' })
    } else {
      ElMessage.error({ __key: response.fail_code ? `errors.${response.fail_code}` : 'files.moveFailed' })
    }
  } catch {
    ElMessage.error({ __key: 'files.moveFailed' })
  }
}

const handleBatchDelete = () => {
  const count = selectedFiles.value.length
  ElMessageBox.confirm(
    t('files.batchDeleteConfirm', { count }),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
      customClass: 'mobile-message-box'
    }
  ).then(async () => {
    const fileNames = selectedFiles.value.map(f => f.name)
    try {
      const res = await batchDeleteFiles(fileNames, currentPath.value || undefined)
      if (res.success) {
        ElMessage.success({ __key: 'files.batchDeleteSuccess', __params: { count } })
      } else {
        const failed = res.data?.failed_files
        if (failed && failed.length > 0) {
          ElMessage.warning({ __key: 'files.batchDeletePartial', __params: { failed: failed.length, total: count } })
        } else {
          ElMessage.error({ __key: 'files.batchDeleteFailed' })
        }
      }
      selectedFiles.value = []
      loadFiles()
    } catch {
      ElMessage.error({ __key: 'files.batchDeleteFailed' })
    }
  }).catch(() => {})
}

const loadFiles = async () => {
  loading.value = true
  try {
    const response = await browseFiles(currentPath.value || undefined)
    fileList.value = response.files
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
    }
  }
  finally {
    loading.value = false
  }
}

watch(currentPath, () => {
  searchKeyword.value = ''
  loadFiles()
})

const handlePathClick = (index: number) => {
  if (index === -1) {
    currentPath.value = ''
  } else {
    currentPath.value = pathList.value.slice(0, index + 1).join('/')
  }
}

const handleFileClick = (row: FileItem) => {
  if (row.file_type === 'parent') {
    // 返回上级目录
    if (currentPath.value) {
      const parts = currentPath.value.split('/').filter(Boolean)
      parts.pop()
      currentPath.value = parts.join('/')
    }
  } else if (row.file_type === 'directory') {
    if (currentPath.value) {
      currentPath.value = `${currentPath.value}/${row.name}`
    } else {
      currentPath.value = row.name
    }
  }
}

const openFileSelector = () => {
  fileInputRef.value?.click()
}

const openFolderSelector = () => {
  folderInputRef.value?.click()
}

const handleFileSelect = (event: Event) => {
  const input = event.target as HTMLInputElement
  const files = input.files
  if (files && files.length > 0) {
    addUploadTasks(Array.from(files), false)
  }
  input.value = ''
}

const handleFolderSelect = (event: Event) => {
  const input = event.target as HTMLInputElement
  const files = input.files
  if (files && files.length > 0) {
    addUploadTasks(Array.from(files), true)
  }
  input.value = ''
}

const addUploadTasks = (files: File[], isFolder: boolean) => {
  const newTasks: UploadTask[] = files.map(file => {
    let relativePath = file.name
    if (isFolder && file.webkitRelativePath) {
      relativePath = file.webkitRelativePath
    }
    if (currentPath.value) {
      relativePath = `${currentPath.value}/${relativePath}`
    }
    return {
      id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      file,
      relativePath,
      size: file.size,
      progress: 0,
      uploadedBytes: 0,
      status: 'waiting' as const
    }
  })
  uploadTasks.value = [...uploadTasks.value, ...newTasks]
  uploadDialogVisible.value = true
  processUploadQueue()
}

const isProcessingQueue = ref(false)

const processUploadQueue = async () => {
  if (isProcessingQueue.value) return
  isProcessingQueue.value = true
  try {
    const waitingTasks = uploadTasks.value.filter(t => t.status === 'waiting')
    const uploadingTasks = uploadTasks.value.filter(t => t.status === 'uploading')
    const availableSlots = 5 - uploadingTasks.length
    
    if (availableSlots > 0 && waitingTasks.length > 0) {
      const tasksToStart = waitingTasks.slice(0, availableSlots)
      for (const task of tasksToStart) {
        task.status = 'uploading'
      }
      for (const task of tasksToStart) {
        startUpload(task)
      }
    }
  } finally {
    isProcessingQueue.value = false
  }
}

const startUpload = async (task: UploadTask) => {
  try {
    const startResponse = await uploadStart([task.relativePath])
    
    if (task.status !== 'uploading') return
    
    if (!startResponse.success) {
      task.status = 'failed'
      task.failCode = startResponse.fail_code
      processUploadQueue()
      return
    }
    
    const uploadInfo = startResponse.uploads?.find(u => u.file === task.relativePath)
    if (!uploadInfo) {
      task.status = 'failed'
      task.failCode = 'INTERNAL_ERROR'
      processUploadQueue()
      return
    }
    
    task.uploadId = uploadInfo.id
  } catch (error: any) {
    if (task.status !== 'uploading') return
    if (error.name === 'CanceledError' || error.code === 'ERR_CANCELED') return
    task.status = 'failed'
    task.failCode = 'NETWORK_ERROR'
    processUploadQueue()
    return
  }
  
  if (task.status !== 'uploading') return
  
  await uploadFileChunks(task)
  
  if (task.status !== 'uploading') return
  
  try {
    const completeResponse = await uploadComplete(task.uploadId!)
    if (completeResponse.success) {
      task.status = 'completed'
      task.progress = 100
    } else {
      task.status = 'failed'
      task.failCode = completeResponse.fail_code
    }
  } catch (error: any) {
    if (task.status !== 'uploading') return
    if (error.name === 'CanceledError' || error.code === 'ERR_CANCELED') return
    task.status = 'failed'
    task.failCode = 'NETWORK_ERROR'
  }
  processUploadQueue()
}

const uploadFileChunks = async (task: UploadTask) => {
  const file = task.file
  let offset = 0
  const chunkSize = optimalChunkSize.value
  
  while (offset < file.size && task.status === 'uploading') {
    const chunk = file.slice(offset, Math.min(offset + chunkSize, file.size))
    const chunkStartOffset = offset
    
    task.abortController = new AbortController()
    
    try {
      const response = await uploadChunk(
        task.uploadId!,
        chunkStartOffset,
        chunk,
        (loaded, _total) => {
          if (task.status === 'uploading') {
            task.uploadedBytes = chunkStartOffset + loaded
            task.progress = Math.min(100, (task.uploadedBytes / file.size) * 100)
          }
        },
        task.abortController.signal
      )
      if (response.success) {
        offset += chunk.size
        task.uploadedBytes = offset
        task.progress = Math.min(100, (offset / file.size) * 100)
      } else {
        task.status = 'failed'
        task.failCode = response.fail_code
        return
      }
    } catch (error: any) {
      if (error.name === 'CanceledError' || error.code === 'ERR_CANCELED') {
        return
      }
      if (task.status !== 'uploading') return
      task.status = 'failed'
      task.failCode = 'NETWORK_ERROR'
      return
    }
  }
}

const cancelAllUploads = async () => {
  for (const task of uploadTasks.value) {
    if (task.status === 'waiting' || task.status === 'uploading') {
      task.status = 'cancelled'
      if (task.abortController) {
        task.abortController.abort()
      }
      if (task.uploadId) {
        try {
          await uploadCancel(task.uploadId)
        } catch {}
      }
    }
  }
}

const getStatusTagType = (status: UploadTask['status']) => {
  switch (status) {
    case 'waiting': return 'info'
    case 'uploading': return 'primary'
    case 'completed': return 'success'
    case 'failed': return 'danger'
    case 'cancelled': return 'warning'
  }
}

const getStatusText = (status: UploadTask['status']) => {
  switch (status) {
    case 'waiting': return t('files.waiting')
    case 'uploading': return t('files.uploading')
    case 'completed': return t('files.completed')
    case 'failed': return t('files.failed')
    case 'cancelled': return t('files.cancelled')
  }
}

const getTaskStatusText = (task: UploadTask) => {
  if (task.status === 'failed' && task.failCode) {
    return `${t('files.failed')} (${getFailCodeText(task.failCode)})`
  }
  return getStatusText(task.status)
}

const getFailCodeText = (failCode: string) => {
  const codeMap: Record<string, string> = {
    'FILES_ALREADY_EXIST': t('files.errorFilesAlreadyExist'),
    'UPLOAD_NOT_FOUND': t('files.errorUploadNotFound'),
    'INVALID_OFFSET': t('files.errorChunkOffsetMismatch'),
    'PATH_NOT_FOUND': t('files.errorPathNotFound'),
    'NOT_LOGGED_IN': t('files.errorNotLoggedIn'),
    'NETWORK_ERROR': t('files.errorNetwork'),
    'INTERNAL_ERROR': t('files.errorInternal'),
  }
  return codeMap[failCode] || failCode
}

const closeUploadDialog = () => {
  uploadDialogVisible.value = false
  uploadTasks.value = uploadTasks.value.filter(t => t.status === 'uploading' || t.status === 'waiting')
  loadFiles()
}

const handleDownload = async (row: FileItem) => {
  const path = currentPath.value ? `${currentPath.value}/${row.name}` : row.name
  
  const abortController = new AbortController()
  
  downloadTask.value = {
    name: row.name,
    path: path,
    size: row.size,
    progress: 0,
    downloadedBytes: 0,
    status: 'downloading',
    abortController: abortController
  }
  downloadDialogVisible.value = true
  
  try {
    const response = await fetchDownloadFile(path, abortController.signal)

    if (!response.ok) {
      throw new Error('Download failed')
    }
    
    const contentLength = response.headers.get('content-length')
    const totalSize = contentLength ? parseInt(contentLength, 10) : row.size
    
    const reader = response.body?.getReader()
    if (!reader) {
      throw new Error('No reader available')
    }
    
    const chunks: BlobPart[] = []
    let downloadedBytes = 0
    
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      
      chunks.push(value)
      downloadedBytes += value.length
      
      if (downloadTask.value) {
        downloadTask.value.downloadedBytes = downloadedBytes
        downloadTask.value.progress = (downloadedBytes / totalSize) * 100
      }
    }
    
    const blob = new Blob(chunks)
    
    if (downloadTask.value) {
      downloadTask.value.blob = blob
      downloadTask.value.status = 'completed'
      downloadTask.value.progress = 100
    }
    
    saveDownloadedFile()
  } catch (error: any) {
    if (error.name === 'AbortError') {
      return
    }
    if (downloadTask.value) {
      downloadTask.value.status = 'failed'
    }
  }
}

const saveDownloadedFile = () => {
  if (!downloadTask.value?.blob) return
  
  const url = URL.createObjectURL(downloadTask.value.blob)
  const a = document.createElement('a')
  a.href = url
  a.download = downloadTask.value.name
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

const redownloadFile = async () => {
  if (!downloadTask.value) return
  
  const task = downloadTask.value
  const abortController = new AbortController()
  
  downloadTask.value = {
    name: task.name,
    path: task.path,
    size: task.size,
    progress: 0,
    downloadedBytes: 0,
    status: 'downloading',
    abortController: abortController
  }
  
  try {
    const response = await fetchDownloadFile(task.path, abortController.signal)
    
    if (!response.ok) {
      throw new Error('Download failed')
    }
    
    const contentLength = response.headers.get('content-length')
    const totalSize = contentLength ? parseInt(contentLength, 10) : task.size
    
    const reader = response.body?.getReader()
    if (!reader) {
      throw new Error('No reader available')
    }
    
    const chunks: BlobPart[] = []
    let downloadedBytes = 0
    
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      
      chunks.push(value)
      downloadedBytes += value.length
      
      if (downloadTask.value) {
        downloadTask.value.downloadedBytes = downloadedBytes
        downloadTask.value.progress = (downloadedBytes / totalSize) * 100
      }
    }
    
    const blob = new Blob(chunks)
    
    if (downloadTask.value) {
      downloadTask.value.blob = blob
      downloadTask.value.status = 'completed'
      downloadTask.value.progress = 100
    }
    
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = task.name
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  } catch (error: any) {
    if (error.name === 'AbortError') {
      return
    }
    if (downloadTask.value) {
      downloadTask.value.status = 'failed'
    }
  }
}

const closeDownloadDialog = () => {
  if (downloadTask.value?.abortController && downloadTask.value.status === 'downloading') {
    downloadTask.value.abortController.abort()
  }
  downloadDialogVisible.value = false
  downloadTask.value = null
}

const handleFolderDownload = async (row: FileItem) => {
  const path = currentPath.value ? `${currentPath.value}/${row.name}` : row.name
  
  folderDownloadTask.value = {
    name: row.name,
    path: path,
    downloadStatus: 'downloading',
    size: 0,
    progress: 0,
    downloadedBytes: 0,
    speed: 0
  }
  folderDownloadDialogVisible.value = true
  
  await doFolderDownload()
}

const doFolderDownload = async () => {
  if (!folderDownloadTask.value) return

  const abortController = new AbortController()
  folderDownloadTask.value.abortController = abortController
  
  try {
    const response = await fetchDownloadFolder(folderDownloadTask.value.path, abortController.signal)
    
    if (!response.ok) {
      throw new Error('Download failed')
    }

    const contentLength = response.headers.get('Content-Length')
    if (contentLength && folderDownloadTask.value) {
      folderDownloadTask.value.size = parseInt(contentLength, 10)
    }
    
    const reader = response.body?.getReader()
    if (!reader) {
      throw new Error('No reader available')
    }
    
    const chunks: BlobPart[] = []
    let downloadedBytes = 0
    let lastSpeedCalcTime = Date.now()
    let lastSpeedCalcBytes = 0
    
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      
      chunks.push(value)
      downloadedBytes += value.length

      const now = Date.now()
      const elapsed = now - lastSpeedCalcTime
      if (elapsed >= 1000 && folderDownloadTask.value) {
        folderDownloadTask.value.speed = Math.round((downloadedBytes - lastSpeedCalcBytes) / (elapsed / 1000))
        lastSpeedCalcTime = now
        lastSpeedCalcBytes = downloadedBytes
      }
      
      if (folderDownloadTask.value) {
        folderDownloadTask.value.downloadedBytes = downloadedBytes
        if (folderDownloadTask.value.size > 0) {
          folderDownloadTask.value.progress = (downloadedBytes / folderDownloadTask.value.size) * 100
        }
      }
    }
    
    const blob = new Blob(chunks)
    
    if (folderDownloadTask.value) {
      folderDownloadTask.value.blob = blob
      folderDownloadTask.value.downloadStatus = 'completed'
      folderDownloadTask.value.downloadedBytes = downloadedBytes
      folderDownloadTask.value.speed = 0
      if (folderDownloadTask.value.size > 0) {
        folderDownloadTask.value.progress = 100
      }
    }
    
    saveFolderDownloadedFile()
  } catch (error: any) {
    if (error.name === 'AbortError') {
      return
    }
    if (folderDownloadTask.value) {
      folderDownloadTask.value.downloadStatus = 'failed'
      folderDownloadTask.value.speed = 0
    }
  }
}

const saveFolderDownloadedFile = () => {
  if (!folderDownloadTask.value?.blob) return
  
  const url = URL.createObjectURL(folderDownloadTask.value.blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${folderDownloadTask.value.name}.zip`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

const retryFolderDownload = async () => {
  if (!folderDownloadTask.value) return

  const task = folderDownloadTask.value

  folderDownloadTask.value = {
    name: task.name,
    path: task.path,
    downloadStatus: 'downloading',
    size: 0,
    progress: 0,
    downloadedBytes: 0,
    speed: 0
  }

  await doFolderDownload()
}

const closeFolderDownloadDialog = () => {
  if (folderDownloadTask.value?.abortController && folderDownloadTask.value.downloadStatus === 'downloading') {
    folderDownloadTask.value.abortController.abort()
  }
  folderDownloadDialogVisible.value = false
  folderDownloadTask.value = null
}

const getFolderDownloadTagType = () => {
  if (!folderDownloadTask.value) return 'info'
  if (folderDownloadTask.value.downloadStatus === 'completed') return 'success'
  if (folderDownloadTask.value.downloadStatus === 'failed') return 'danger'
  if (folderDownloadTask.value.downloadStatus === 'downloading') return 'primary'
  return 'info'
}

const getFolderDownloadStatusText = () => {
  if (!folderDownloadTask.value) return ''
  if (folderDownloadTask.value.downloadStatus === 'completed') return t('files.downloadCompleted')
  if (folderDownloadTask.value.downloadStatus === 'failed') return t('files.downloadFailed')
  if (folderDownloadTask.value.downloadStatus === 'downloading') return t('files.downloading')
  return ''
}

const handleNewFolder = () => {
  newFolderName.value = ''
  newFolderDialogVisible.value = true
}

const confirmCreateFolder = async () => {
  if (!newFolderName.value.trim()) {
    ElMessage.warning({ __key: 'files.pleaseEnterFolderName' })
    return
  }
  try {
    await createFolder(currentPath.value || undefined, newFolderName.value.trim())
    ElMessage.success({ __key: 'files.createFolderSuccess', __params: { name: newFolderName.value } })
    newFolderDialogVisible.value = false
    loadFiles()
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
    }
  }
}

const handleDelete = (row: FileItem) => {
  ElMessageBox.confirm(
    t('files.deleteConfirm', { name: row.name }),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
      customClass: 'mobile-message-box'
    }
  ).then(async () => {
    const path = currentPath.value ? `${currentPath.value}/${row.name}` : row.name
    try {
      await deleteFile(path)
      ElMessage.success({ __key: 'files.deleteSuccess', __params: { name: row.name } })
      loadFiles()
    } catch (error: any) {
      if (error.isAxiosError && !error.response) {
        ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
      }
    }
  }).catch(() => {})
}

const sortByName = (a: FileItem, b: FileItem): number => {
  if (a.file_type === 'parent') return -1
  if (b.file_type === 'parent') return 1
  return a.name.localeCompare(b.name)
}

const sortBySize = (a: FileItem, b: FileItem): number => {
  if (a.file_type === 'parent') return -1
  if (b.file_type === 'parent') return 1
  return a.size - b.size
}

const sortByModified = (a: FileItem, b: FileItem): number => {
  if (a.file_type === 'parent') return -1
  if (b.file_type === 'parent') return 1
  const ta = a.modified ? Number(a.modified) : 0
  const tb = b.modified ? Number(b.modified) : 0
  return ta - tb
}

const handleShare = async (row: FileItem) => {
  const path = currentPath.value ? `${currentPath.value}/${row.name}` : row.name
  shareFilePath.value = path
  shareFileName.value = row.name
  existingShare.value = null
  shareForm.value = {
    expire_type: 'permanent',
    expire_at: null,
    max_downloads: 10,
    share_mode: 'page',
    usePassword: false,
    password: '',
  }
  shareDrawerVisible.value = true
  shareDrawerLoading.value = true

  try {
    const res = await getShareByPath({ file_path: path })
    if (res.success && res.share) {
      existingShare.value = res.share
    }
  } catch {
  } finally {
    shareDrawerLoading.value = false
  }
}

const copyShareLink = async () => {
  if (!existingShare.value) return
  try {
    const url = existingShare.value.share_mode === 'direct'
      ? `${window.location.origin}/api/share/file/${existingShare.value.share_code}`
      : `${window.location.origin}/s/${existingShare.value.share_code}`
    await navigator.clipboard.writeText(url)
    ElMessage.success({ __key: 'share.linkCopied' })
  } catch {}
}

const handleCancelShare = () => {
  if (!existingShare.value) return
  ElMessageBox.confirm(
    t('share.confirmCancelShare'),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
      customClass: 'mobile-message-box'
    }
  ).then(async () => {
    try {
      await deleteShares({ ids: [existingShare.value!.id] })
      ElMessage.success({ __key: 'share.shareCancelled' })
      existingShare.value = null
      shareDrawerVisible.value = false
      const info = await getSystemInfo(true)
      userStore.setSystemInfo(info)
    } catch {}
  }).catch(() => {})
}

const handleCreateShare = async () => {
  shareCreating.value = true
  try {
    const res = await createShare({
      file_path: shareFilePath.value,
      expire_type: shareForm.value.expire_type,
      expire_at: shareForm.value.expire_type === 'time' && shareForm.value.expire_at ? new Date(shareForm.value.expire_at).toISOString() : null,
      max_downloads: shareForm.value.expire_type === 'count' ? shareForm.value.max_downloads : null,
      share_mode: shareForm.value.share_mode,
      password: shareForm.value.usePassword ? shareForm.value.password : null,
    })
    if (res.success && res.share_code) {
      ElMessage.success({ __key: 'share.shareCreated' })
      userStore.setHasShares(true)
      try {
        const shareRes = await getShareByPath({ file_path: shareFilePath.value })
        if (shareRes.success && shareRes.share) {
          existingShare.value = shareRes.share
        }
      } catch {}
    } else if (res.fail_code) {
      ElMessage.error({ __key: `errors.${res.fail_code}` })
    }
  } catch {
    ElMessage.error({ __key: 'errors.INTERNAL_ERROR' })
  } finally {
    shareCreating.value = false
  }
}
</script>

<style scoped>
.files-container {
  padding: 20px;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.files-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  gap: 15px;
  flex-shrink: 0;
}

.table-wrapper {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.selection-bar {
  position: absolute;
  bottom: 1px;
  left: 1px;
  right: 1px;
  background: var(--el-bg-color);
  padding: 12px 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  z-index: 10;
  box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.08);
}

.dark .selection-bar {
  box-shadow: 0 -2px 8px rgba(255, 255, 255, 0.05);
}

.selection-count {
  font-size: 14px;
  color: var(--el-text-color-regular);
}

.selection-actions {
  display: flex;
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.3s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100%);
  opacity: 0;
}

.breadcrumb-wrapper {
  width: 0;
  flex: 1;
  overflow-x: auto;
  overflow-y: hidden;
}

.path-breadcrumb {
  background: var(--el-fill-color-light);
  padding: 8px 16px;
  border-radius: 6px;
  display: flex;
  flex-wrap: nowrap;
}

.breadcrumb-item-link {
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--el-text-color-regular);
  transition: color 0.2s;
  white-space: nowrap;
}

.breadcrumb-item-link:hover {
  color: var(--el-color-primary);
}

.toolbar-actions {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.search-input {
  width: 250px;
}

.file-name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.file-name-cell.clickable {
  cursor: pointer;
}

.file-icon {
  font-size: 20px;
  color: var(--el-color-primary);
}

.file-icon.is-folder {
  color: #e6a23c;
}

.file-icon.is-parent {
  color: #909399;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn-text {
  margin-left: 6px;
}

.upload-list {
  max-height: 400px;
  overflow-y: auto;
}

.upload-item {
  padding: 12px;
  margin-bottom: 12px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
}

.upload-item:last-child {
  margin-bottom: 0;
}

.upload-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.upload-item-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
  margin-right: 12px;
}

.upload-item-info {
  display: flex;
  justify-content: space-between;
  margin-bottom: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.upload-item-progress {
  color: var(--el-color-primary);
  font-weight: 500;
}

.upload-empty {
  text-align: center;
  padding: 40px 0;
  color: var(--el-text-color-secondary);
}

.upload-dialog-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.upload-summary {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}

.upload-actions {
  display: flex;
}

.progress-section {
  margin-bottom: 16px;
}

.progress-section:last-child {
  margin-bottom: 0;
}

.progress-label {
  font-size: 13px;
  color: var(--el-text-color-regular);
  margin-bottom: 4px;
}

.download-item {
  padding: 12px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
}

.move-dialog-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.move-info {
  display: flex;
  gap: 8px;
  font-size: 14px;
}

.move-info span:first-child {
  color: var(--el-text-color-secondary);
}

.move-path {
  color: var(--el-text-color-primary);
  font-weight: 500;
}

.move-browser {
  border: 1px solid var(--el-border-color);
  border-radius: 8px;
  overflow: hidden;
}

.move-browser-header {
  padding: 8px 12px;
  background: var(--el-fill-color-light);
  border-bottom: 1px solid var(--el-border-color);
}

.move-browser-list {
  height: 250px;
  overflow-y: auto;
  padding: 8px;
}

.move-folder-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.move-folder-item:hover {
  background: var(--el-fill-color-light);
}

.move-folder-item .el-icon {
  font-size: 18px;
  color: #e6a23c;
}

.move-empty {
  text-align: center;
  padding: 40px 0;
  color: var(--el-text-color-secondary);
}

/* Mobile Layout Adjustments */
.files-container.is-mobile {
  padding: 12px 8px;
}

.files-container.is-mobile .files-toolbar {
  margin-bottom: 12px;
  gap: 8px;
}

.files-container.is-mobile .breadcrumb-wrapper {
  display: none;
}

.files-container.is-mobile .toolbar-actions {
  justify-content: flex-start;
  gap: 8px;
}

.files-container.is-mobile .search-input {
  display: none !important;
}

.share-drawer-loading {
  display: flex;
  justify-content: center;
  padding: 40px 0;
}

.share-info-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.share-info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.share-info-label {
  color: var(--el-text-color-secondary);
  font-size: 14px;
}

.share-info-value {
  font-size: 14px;
  color: var(--el-text-color-primary);
  word-break: break-all;
  text-align: right;
}

.share-link-text {
  flex: 1;
  min-width: 0;
  margin-right: 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.share-copy-btn {
  flex-shrink: 0;
}

.share-info-actions {
  display: flex;
  gap: 8px;
}

.share-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.share-form-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.share-form-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.share-form-label {
  font-size: 14px;
  color: var(--el-text-color-regular);
  font-weight: 500;
}

.share-form-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.4;
}
</style>

<style>
/* Mobile MessageBox - 全局样式，不能用 scoped */
@media (max-width: 767px) {
  .mobile-message-box {
    width: calc(100vw - 32px) !important;
    max-width: 400px !important;
  }
}
</style>
