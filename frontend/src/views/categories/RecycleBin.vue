<template>
  <div class="recycle-bin-container" :class="{ 'is-mobile': isMobileLayout }">
    <div class="recycle-toolbar">
      <div class="toolbar-left">
        <el-button type="danger" @click="handleEmptyRecycleBin" :disabled="total === 0">
          <span class="btn-text">{{ t('recycleBin.emptyRecycleBin') }}</span>
        </el-button>
      </div>
      <div class="toolbar-right">
        <el-input
          v-if="!isMobileLayout"
          v-model="searchKeyword"
          class="search-input"
          :placeholder="t('recycleBin.searchPlaceholderCurrentPage')"
          :prefix-icon="Search"
          clearable
        />
      </div>
    </div>

    <div class="table-wrapper">
      <el-table
        :data="displayItems"
        style="width: 100%"
        stripe
        border
        height="100%"
        show-overflow-tooltip
        v-loading="loading"
        @selection-change="handleSelectionChange"
      >
        <el-table-column type="selection" width="38" />
        <el-table-column :label="t('recycleBin.originalPath')" min-width="300" prop="original_path">
          <template #default="{ row }">
            <div class="path-cell">
              <el-icon class="file-icon" :class="row.is_directory ? 'is-folder' : ''">
                <Folder v-if="row.is_directory" />
                <Document v-else />
              </el-icon>
              <span>{{ row.original_path }}</span>
            </div>
          </template>
        </el-table-column>

        <el-table-column :label="t('files.fileSize')" width="120" prop="file_size" sortable :sort-method="sortBySize">
          <template #default="{ row }">
            {{ formatFileSize(row.file_size) }}
          </template>
        </el-table-column>

        <el-table-column :label="t('recycleBin.deletedTime')" width="180" prop="deleted_at" sortable :sort-method="sortByDeletedAt">
          <template #default="{ row }">
            {{ formatUtcDatetimeString(row.deleted_at) }}
          </template>
        </el-table-column>

        <el-table-column :label="t('files.operations')" width="160" fixed="right">
          <template #default="{ row }">
            <el-link type="primary" @click="handleRestore(row)">
              {{ t('recycleBin.restore') }}
            </el-link>
            <el-link type="danger" @click="handlePermanentDelete(row)">
              {{ t('recycleBin.permanentDelete') }}
            </el-link>
          </template>
        </el-table-column>
      </el-table>

      <transition name="slide-up">
        <div v-if="selectedItems.length > 0" class="selection-bar">
          <span class="selection-count">{{ t('recycleBin.selectedCount', { count: selectedItems.length }) }}</span>
          <div class="selection-actions">
            <el-button type="primary" @click="handleBatchRestore">
              {{ t('recycleBin.restore') }}
            </el-button>
            <el-button type="danger" @click="handleBatchPermanentDelete">
              {{ t('recycleBin.permanentDelete') }}
            </el-button>
          </div>
        </div>
      </transition>
    </div>

    <div class="pagination-wrapper" v-if="total > 0">
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[20, 50, 100]"
        :total="total"
        layout="total, sizes, prev, pager, next"
        background
        @size-change="handlePageSizeChange"
        @current-change="handlePageChange"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox } from 'element-plus'
import { Search, Folder, Document } from '@element-plus/icons-vue'
import { formatUtcDatetimeString } from '@/utils/date'
import { formatFileSize } from '@/utils/format'
import {
  getRecycleBinList,
  restoreRecycleItem,
  batchRestoreRecycleItems,
  deleteRecycleItem,
  batchDeleteRecycleItems,
  emptyRecycleBin,
  type RecycleBinItem,
} from '@/api/system'

const { t } = useI18n()

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
  loadList()
})

onUnmounted(() => {
  window.removeEventListener('resize', checkLayout)
})

const loading = ref(false)
const items = ref<RecycleBinItem[]>([])
const total = ref(0)
const currentPage = ref(1)
const pageSize = ref(20)
const searchKeyword = ref('')
const selectedItems = ref<RecycleBinItem[]>([])

const displayItems = computed(() => {
  if (!searchKeyword.value) return items.value
  return items.value.filter(item =>
    item.original_path.toLowerCase().includes(searchKeyword.value.toLowerCase())
  )
})

const handleSelectionChange = (selection: RecycleBinItem[]) => {
  selectedItems.value = selection
}

let loadingCount = 0

const loadList = async () => {
  loadingCount++
  loading.value = true
  try {
    const response = await getRecycleBinList({
      page: currentPage.value,
      page_size: pageSize.value,
    })
    items.value = response.data?.items ?? []
    total.value = response.data?.total ?? 0
  } catch {
  } finally {
    loadingCount--
    if (loadingCount === 0) {
      loading.value = false
    }
  }
}

const handlePageChange = () => {
  searchKeyword.value = ''
  loadList()
}

const handlePageSizeChange = () => {
  currentPage.value = 1
  searchKeyword.value = ''
  loadList()
}

const sortBySize = (a: RecycleBinItem, b: RecycleBinItem): number => {
  return a.file_size - b.file_size
}

const sortByDeletedAt = (a: RecycleBinItem, b: RecycleBinItem): number => {
  return new Date(a.deleted_at).getTime() - new Date(b.deleted_at).getTime()
}

const handleRestore = async (row: RecycleBinItem) => {
  try {
    await restoreRecycleItem(row.id)
    ElMessage.success({ __key: 'recycleBin.restoreSuccess' })
    loadList()
  } catch {}
}

const handlePermanentDelete = (row: RecycleBinItem) => {
  ElMessageBox.confirm(
    t('recycleBin.permanentDeleteConfirm'),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
      customClass: 'mobile-message-box'
    }
  ).then(async () => {
    try {
      await deleteRecycleItem(row.id)
      ElMessage.success({ __key: 'recycleBin.deleteSuccess' })
      loadList()
    } catch {}
  }).catch(() => {})
}

const handleBatchRestore = async () => {
  const ids = selectedItems.value.map(item => item.id)
  try {
    const res = await batchRestoreRecycleItems(ids)
    if (!res.success && res.fail_code === 'RESTORE_PATH_OCCUPIED' && res.data?.conflict_items?.length) {
      const lines = res.data.conflict_items.map(item =>
        `${item.is_directory ? t('recycleBin.directory') : t('recycleBin.file')} ${item.original_path} ${t('recycleBin.occupied')}`
      )
      ElMessage.error(lines.join('\n'))
    } else if (!res.success && res.fail_code) {
      ElMessage.error({ __key: `errors.${res.fail_code}` })
    } else {
      ElMessage.success({ __key: 'recycleBin.restoreSuccess' })
      selectedItems.value = []
      loadList()
    }
  } catch {}
}

const handleBatchPermanentDelete = () => {
  const count = selectedItems.value.length
  ElMessageBox.confirm(
    t('recycleBin.batchPermanentDeleteConfirm', { count }),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
      customClass: 'mobile-message-box'
    }
  ).then(async () => {
    const ids = selectedItems.value.map(item => item.id)
    try {
      const res = await batchDeleteRecycleItems(ids)
      if (!res.success && res.fail_code === 'DELETE_FAILED' && res.data?.failed_paths?.length) {
        ElMessage.error(res.data.failed_paths.join('\n'))
      } else if (!res.success && res.fail_code) {
        ElMessage.error({ __key: `errors.${res.fail_code}` })
      } else {
        ElMessage.success({ __key: 'recycleBin.deleteSuccess' })
        selectedItems.value = []
      }
      loadList()
    } catch {}
  }).catch(() => {})
}

const handleEmptyRecycleBin = () => {
  ElMessageBox.confirm(
    t('recycleBin.emptyConfirm'),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
      customClass: 'mobile-message-box'
    }
  ).then(async () => {
    try {
      await emptyRecycleBin()
      ElMessage.success({ __key: 'recycleBin.emptySuccess' })
      loadList()
    } catch {}
  }).catch(() => {})
}
</script>

<style scoped>
.recycle-bin-container {
  padding: 20px;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.recycle-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
}

.toolbar-right {
  display: flex;
  align-items: center;
}

.search-input {
  width: 250px;
}

.table-wrapper {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.path-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.file-icon {
  font-size: 20px;
  color: var(--el-color-primary);
  flex-shrink: 0;
}

.file-icon.is-folder {
  color: #e6a23c;
}

.btn-text {
  margin-left: 6px;
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

.pagination-wrapper {
  display: flex;
  justify-content: flex-end;
  padding: 16px 0 4px;
  flex-shrink: 0;
}

.recycle-bin-container.is-mobile {
  padding: 12px 8px;
}

.recycle-bin-container.is-mobile .recycle-toolbar {
  margin-bottom: 12px;
}

.recycle-bin-container.is-mobile .search-input {
  display: none !important;
}

.recycle-bin-container.is-mobile .pagination-wrapper {
  justify-content: center;
}

.recycle-bin-container.is-mobile .btn-text {
  margin-left: 0;
}
</style>
