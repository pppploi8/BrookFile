<template>
  <el-drawer
    v-model="visible"
    :title="t('backupLog.title')"
    direction="rtl"
    :size="isMobile ? '100%' : '600px'"
    destroy-on-close
    class="backup-log-drawer"
    @opened="handleDrawerOpened"
    @closed="handleDrawerClosed"
  >
    <div class="drawer-body-wrapper">
      <div class="toolbar">
        <template v-if="!isTaskRunning">
          <el-button type="primary" @click="handleBackupNow" :loading="startingBackup">
            {{ t('backupLog.backupNow') }}
          </el-button>
          <el-button @click="handleCleanup" :loading="startingCleanup">
            {{ t('backupLog.cleanup') }}
          </el-button>
        </template>
        <el-button v-else type="danger" @click="handleCancelTask" :loading="cancellingTask">
          {{ t('backupLog.cancelTask') }}
        </el-button>
      </div>

      <el-tabs v-model="activeTab" class="log-tabs">
        <el-tab-pane :label="t('backupLog.executionStatus')" name="status">
          <div class="status-content">
            <div class="task-info" v-if="isTaskRunning">
              {{ t('backupLog.currentTask') }}: {{ currentPhase === 'backup' ? t('backupLog.backupTask') : t('backupLog.cleanupTask') }}<template v-if="currentSubPhase === 'scanning'">({{ t('backupLog.scannedBytes', { size: formatScannedBytes(scannedBytes) }) }})</template><template v-else-if="currentSubPhase">({{ getSubPhaseText(currentSubPhase) }})</template>
            </div>
            <div class="task-info" v-else>
              {{ t('backupLog.noTask') }}
            </div>
            <div class="table-wrapper" v-if="isTaskRunning">
              <el-table :data="statusData" height="100%" v-loading="loadingStatus">
                <el-table-column prop="name" :label="t('backupLog.fileName')" min-width="200" show-overflow-tooltip />
                <el-table-column :label="t('backupLog.progress')" width="150">
                  <template #default="{ row }">
                    <el-progress :percentage="row.progress" :stroke-width="8" />
                  </template>
                </el-table-column>
                <el-table-column prop="status" :label="t('backupLog.status')" width="140" show-overflow-tooltip>
                  <template #default="{ row }">
                    <span :class="['status-tag', `status-${row.status}`]">
                      {{ getStatusText(row.status) }}
                    </span>
                  </template>
                </el-table-column>
              </el-table>
            </div>
            <el-empty v-else :description="t('backupLog.noTaskRunning')" />
          </div>
        </el-tab-pane>

        <el-tab-pane :label="t('backupLog.historyLog')" name="history">
          <div class="history-content">
            <div class="table-wrapper">
              <el-table :data="historyData" height="100%" border stripe v-loading="loadingHistory">
                <el-table-column prop="started_at" :label="t('backupLog.startTime')" width="180" />
                <el-table-column prop="backup_success_count" :label="t('backupLog.successCount')" width="120" />
                <el-table-column prop="backup_fail_count" :label="t('backupLog.failCount')" width="120" />
                <el-table-column prop="cleanup_deleted_count" :label="t('backupLog.cleanupCount')" width="120" />
                <el-table-column :label="t('backupLog.status')" width="100">
                  <template #default="{ row }">
                    <span :class="['status-tag', `status-${row.status}`]">
                      {{ getLogStatusText(row.status) }}
                    </span>
                  </template>
                </el-table-column>
                <el-table-column prop="fail_reason" :label="t('backupLog.failReason')" min-width="150" show-overflow-tooltip />
                <el-table-column prop="finished_at" :label="t('backupLog.endTime')" width="180" />
              </el-table>
            </div>
            <div class="pagination-wrapper">
              <el-pagination
                v-model:current-page="currentPage"
                v-model:page-size="pageSize"
                :page-sizes="[10, 20, 50]"
                :total="totalLogs"
                layout="total, sizes, prev, pager, next"
                @size-change="handleSizeChange"
                @current-change="handlePageChange"
              />
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </div>
  </el-drawer>
</template>

<script setup lang="ts">
import { ref, onUnmounted, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import {
  startBackup,
  cancelBackup,
  getBackupProgress,
  getBackupLogs,
  type BackupProgressItem,
  type BackupLogItem,
} from '@/api/system'

const { t } = useI18n()

const visible = ref(false)
const activeTab = ref('status')
const currentRuleId = ref('')

const isTaskRunning = ref(false)
const currentPhase = ref<'backup' | 'cleanup' | null>(null)
const currentSubPhase = ref<'scanning' | null>(null)
const scannedBytes = ref(0)
const loadingStatus = ref(false)
const isInitialLoad = ref(true)
const startingBackup = ref(false)
const startingCleanup = ref(false)
const cancellingTask = ref(false)

const isMobile = ref(window.innerWidth <= 768)

const statusData = ref<Array<{
  name: string
  progress: number
  status: string
  error?: string
}>>([])

const historyData = ref<BackupLogItem[]>([])
const loadingHistory = ref(false)
const currentPage = ref(1)
const pageSize = ref(20)
const totalLogs = ref(0)

let progressTimer: ReturnType<typeof setInterval> | null = null

watch(activeTab, (newVal) => {
  if (newVal === 'history') {
    loadBackupLogs()
  }
})

const getStatusText = (status: string) => {
  if (status.startsWith('waiting_retry')) {
    const match = status.match(/waiting_retry \((\d+)\/(\d+)\)/)
    if (match) {
      return `${t('backupLog.statusWaitingRetry')} (${match[1]}/${match[2]})`
    }
    return t('backupLog.statusWaitingRetry')
  }
  if (status.startsWith('retrying')) {
    const match = status.match(/retrying \((\d+)\/(\d+)\)/)
    if (match) {
      return `${t('backupLog.statusRetrying')} (${match[1]}/${match[2]})`
    }
    return t('backupLog.statusRetrying')
  }
  const texts: Record<string, string> = {
    pending: t('backupLog.statusPending'),
    uploading: t('backupLog.statusUploading'),
    cleaning: t('backupLog.statusCleaning'),
    completed: t('backupLog.statusCompleted'),
    failed: t('backupLog.statusFailed'),
  }
  return texts[status] || status
}

const getSubPhaseText = (subPhase: string) => {
  const texts: Record<string, string> = {
    scanning: t('backupLog.statusScanning'),
  }
  return texts[subPhase] || subPhase
}

const formatScannedBytes = (bytes: number): string => {
  const mb = 1024 * 1024
  const kb = 1024
  if (bytes >= mb) {
    return `${(bytes / mb).toFixed(1)} MB`
  } else if (bytes >= kb) {
    return `${(bytes / kb).toFixed(1)} KB`
  } else {
    return `${bytes} B`
  }
}

const getLogStatusText = (status: string) => {
  const texts: Record<string, string> = {
    completed: t('backupLog.statusCompleted'),
    failed: t('backupLog.statusFailed'),
    cancelled: t('backupLog.statusCancelled'),
    interrupted: t('backupLog.statusInterrupted'),
  }
  return texts[status] || status
}

const open = (ruleId: string) => {
  currentRuleId.value = ruleId
  activeTab.value = 'status'
  isInitialLoad.value = true
  loadBackupProgress()
  loadBackupLogs()
  visible.value = true
}

const handleDrawerOpened = () => {
  startProgressPolling()
}

const handleDrawerClosed = () => {
  stopProgressPolling()
}

const startProgressPolling = () => {
  if (progressTimer) {
    clearInterval(progressTimer)
  }
  progressTimer = setInterval(() => {
    loadBackupProgress()
  }, 1000)
}

const stopProgressPolling = () => {
  if (progressTimer) {
    clearInterval(progressTimer)
    progressTimer = null
  }
}

const loadBackupProgress = async () => {
  if (!currentRuleId.value) return
  if (isInitialLoad.value) {
    loadingStatus.value = true
  }
  try {
    const data = await getBackupProgress({ rule_id: currentRuleId.value })
    isTaskRunning.value = data.is_running
    currentPhase.value = data.phase
    currentSubPhase.value = data.sub_phase
    scannedBytes.value = data.scanned_bytes || 0
    if (data.is_running && data.pending_items && data.pending_items.length > 0) {
      statusData.value = data.pending_items.map((item: BackupProgressItem) => ({
        name: item.name,
        status: item.status,
        progress: item.total_bytes > 0 ? Math.round((item.uploaded_bytes / item.total_bytes) * 100) : 0,
        error: item.error,
      }))
    } else {
      statusData.value = []
    }
  } catch {
  } finally {
    loadingStatus.value = false
    isInitialLoad.value = false
  }
}

const loadBackupLogs = async () => {
  if (!currentRuleId.value) return
  loadingHistory.value = true
  try {
    const data = await getBackupLogs({
      rule_id: currentRuleId.value,
      page: currentPage.value,
      page_size: pageSize.value,
    })
    historyData.value = data.items
    totalLogs.value = data.total
  } catch {
  } finally {
    loadingHistory.value = false
  }
}

const handleBackupNow = async () => {
  if (!currentRuleId.value) return
  startingBackup.value = true
  try {
    await startBackup({ rule_id: currentRuleId.value, mode: 'full' })
    ElMessage.success({ __key: 'backupLog.startBackupSuccess' })
    await loadBackupProgress()
    startProgressPolling()
  } catch {
  } finally {
    startingBackup.value = false
  }
}

const handleCleanup = async () => {
  if (!currentRuleId.value) return
  startingCleanup.value = true
  try {
    await startBackup({ rule_id: currentRuleId.value, mode: 'cleanup_only' })
    ElMessage.success({ __key: 'backupLog.startCleanupSuccess' })
    await loadBackupProgress()
    startProgressPolling()
  } catch {
  } finally {
    startingCleanup.value = false
  }
}

const handleCancelTask = async () => {
  if (!currentRuleId.value) return
  cancellingTask.value = true
  try {
    await cancelBackup({ rule_id: currentRuleId.value })
    ElMessage.success({ __key: 'backupLog.cancelTaskSuccess' })
    await loadBackupProgress()
  } catch {
  } finally {
    cancellingTask.value = false
  }
}

const handleSizeChange = (val: number) => {
  pageSize.value = val
  currentPage.value = 1
  loadBackupLogs()
}

const handlePageChange = (val: number) => {
  currentPage.value = val
  loadBackupLogs()
}

const handleResize = () => {
  isMobile.value = window.innerWidth <= 768
}

onMounted(() => {
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  stopProgressPolling()
  window.removeEventListener('resize', handleResize)
})

defineExpose({ open })
</script>

<style scoped>
.backup-log-drawer :deep(.el-drawer__body) {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 0;
  overflow: hidden;
}

.drawer-body-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.toolbar {
  margin-bottom: 16px;
  flex-shrink: 0;
}

.log-tabs {
  flex: 1;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
}

.log-tabs :deep(.el-tabs__header) {
  flex-shrink: 0;
  margin-bottom: 16px;
}

.log-tabs :deep(.el-tabs__content) {
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.log-tabs :deep(.el-tab-pane) {
  height: 100%;
  min-height: 0;
}

.status-content {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.history-content {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.task-info {
  margin-bottom: 16px;
  padding: 12px 16px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  flex-shrink: 0;
}

.table-wrapper {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.table-wrapper :deep(.el-table .cell) {
  white-space: nowrap;
}

.pagination-wrapper {
  margin-top: 16px;
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
}

.status-tag {
  font-size: 12px;
  font-weight: 500;
}

.status-tag.status-pending {
  color: var(--el-text-color-secondary);
}

.status-tag.status-uploading,
.status-tag.status-running {
  color: var(--el-color-primary);
}

.status-tag.status-waiting_retry {
  color: var(--el-color-warning);
}

.status-tag.status-retrying {
  color: var(--el-color-primary);
}

.status-tag.status-cleaning {
  color: var(--el-color-warning);
}

.status-tag.status-success,
.status-tag.status-completed {
  color: var(--el-color-success);
}

.status-tag.status-failed {
  color: var(--el-color-danger);
}

.status-tag.status-cancelled,
.status-tag.status-interrupted {
  color: var(--el-text-color-secondary);
}

.error-text {
  color: var(--el-color-danger);
  font-size: 12px;
}
</style>
