<template>
  <div class="shares-container" :class="{ 'is-mobile': isMobileLayout }">
    <div class="table-wrapper">
      <el-table
        :data="shares"
        style="width: 100%"
        stripe
        border
        height="100%"
        show-overflow-tooltip
        v-loading="loading"
        @selection-change="handleSelectionChange"
        :empty-text="t('share.noShares')"
      >
        <el-table-column type="selection" width="38" />
        <el-table-column :label="t('share.fileName')" min-width="200" prop="file_name">
          <template #default="{ row }">
            <div class="name-cell">
              <el-icon class="file-icon" :class="row.is_directory ? 'is-folder' : ''">
                <Folder v-if="row.is_directory" />
                <Document v-else />
              </el-icon>
              <span>{{ row.file_name }}</span>
            </div>
          </template>
        </el-table-column>

        <el-table-column :label="t('share.fileType')" width="100">
          <template #default="{ row }">
            {{ row.is_directory ? t('share.folder') : t('share.file') }}
          </template>
        </el-table-column>

        <el-table-column :label="t('share.shareMode')" width="100">
          <template #default="{ row }">
            {{ row.share_mode === 'direct' ? t('share.directLink') : t('share.downloadPage') }}
          </template>
        </el-table-column>

        <el-table-column :label="t('share.expireType')" width="180">
          <template #default="{ row }">
            <span v-if="row.expire_type === 'permanent'">{{ t('share.permanent') }}</span>
            <span v-else-if="row.expire_type === 'time'">{{ formatUtcDatetimeString(row.expire_at!) }}</span>
            <span v-else-if="row.expire_type === 'count'">{{ row.download_count }} / {{ row.max_downloads }} {{ t('share.times') }}</span>
          </template>
        </el-table-column>

        <el-table-column :label="t('share.downloadCount')" width="120">
          <template #default="{ row }">
            {{ row.max_downloads != null ? `${row.download_count} / ${row.max_downloads}` : row.download_count }}
          </template>
        </el-table-column>

        <el-table-column :label="t('share.hasPassword')" width="100">
          <template #default="{ row }">
            {{ row.has_password ? t('share.yes') : t('share.no') }}
          </template>
        </el-table-column>

        <el-table-column :label="t('share.status')" width="100">
          <template #default="{ row }">
            <el-tag v-if="row.status === 'active'" type="success">{{ t('share.active') }}</el-tag>
            <el-tag v-else-if="row.status === 'expired'" type="info">{{ t('share.expired') }}</el-tag>
            <el-tag v-else-if="row.status === 'file_missing'" type="danger">{{ t('share.fileMissing') }}</el-tag>
          </template>
        </el-table-column>

        <el-table-column :label="t('share.createdTime')" width="180" prop="created_at">
          <template #default="{ row }">
            {{ formatUtcDatetimeString(row.created_at) }}
          </template>
        </el-table-column>

        <el-table-column :label="t('files.operations')" :width="isMobileLayout ? 80 : 160" fixed="right">
          <template #default="{ row }">
            <el-link type="primary" :icon="Link" @click="handleCopyLink(row)">
              <span v-if="!isMobileLayout">{{ t('share.copyLink') }}</span>
            </el-link>
            <el-link type="danger" :icon="Delete" @click="handleDelete(row)">
              <span v-if="!isMobileLayout">{{ t('share.deleteShare') }}</span>
            </el-link>
          </template>
        </el-table-column>
      </el-table>

      <transition name="slide-up">
        <div v-if="selectedItems.length > 0" class="selection-bar">
          <span class="selection-count">{{ t('recycleBin.selectedCount', { count: selectedItems.length }) }}</span>
          <div class="selection-actions">
            <el-button type="danger" @click="handleBatchDelete">
              {{ t('share.batchDelete') }}
            </el-button>
          </div>
        </div>
      </transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox } from 'element-plus'
import { Folder, Document, Link, Delete } from '@element-plus/icons-vue'
import { formatUtcDatetimeString } from '@/utils/date'
import { listShares, deleteShares, getSystemInfo, type ShareItem } from '@/api/system'
import { useUserStore } from '@/stores/user'

const { t } = useI18n()
const userStore = useUserStore()

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
  loadShares()
})

onUnmounted(() => {
  window.removeEventListener('resize', checkLayout)
})

const loading = ref(false)
const shares = ref<ShareItem[]>([])
const selectedItems = ref<ShareItem[]>([])

const handleSelectionChange = (selection: ShareItem[]) => {
  selectedItems.value = selection
}

const loadShares = async () => {
  loading.value = true
  try {
    const res = await listShares()
    if (res.success) {
      shares.value = res.shares ?? []
    }
  } catch {
  } finally {
    loading.value = false
  }
}

const handleCopyLink = async (row: ShareItem) => {
  try {
    const url = row.share_mode === 'direct'
      ? `${window.location.origin}/api/share/file/${row.share_code}`
      : `${window.location.origin}/s/${row.share_code}`
    await navigator.clipboard.writeText(url)
    ElMessage.success({ __key: 'share.linkCopied' })
  } catch {}
}

const handleDelete = (row: ShareItem) => {
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
      await deleteShares({ ids: [row.id] })
      ElMessage.success({ __key: 'share.shareCancelled' })
      const info = await getSystemInfo(true)
      userStore.setSystemInfo(info)
      loadShares()
    } catch {}
  }).catch(() => {})
}

const handleBatchDelete = () => {
  ElMessageBox.confirm(
    t('share.confirmDeleteShares'),
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
      await deleteShares({ ids })
      ElMessage.success({ __key: 'share.shareCancelled' })
      selectedItems.value = []
      const info = await getSystemInfo(true)
      userStore.setSystemInfo(info)
      loadShares()
    } catch {}
  }).catch(() => {})
}
</script>

<style scoped>
.shares-container {
  padding: 20px;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.shares-toolbar {
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

.table-wrapper {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.name-cell {
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

.shares-container.is-mobile {
  padding: 12px 8px;
}

.shares-container.is-mobile .shares-toolbar {
  margin-bottom: 12px;
}

.shares-container.is-mobile .btn-text {
  margin-left: 0;
}

.shares-container.is-mobile .el-link .el-icon {
  margin-right: 0;
}
</style>
