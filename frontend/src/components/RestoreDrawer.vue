<template>
  <el-drawer
    v-model="visible"
    :title="t('restore.title')"
    direction="rtl"
    :size="isMobile ? '100%' : '450px'"
    destroy-on-close
    class="restore-drawer"
    :before-close="handleBeforeClose"
  >
    <div class="drawer-body-wrapper">
      <el-steps :active="currentStep + (restoreCompleted ? 1 : 0)" align-center class="restore-steps">
        <el-step :title="t('restore.step1Title')" />
        <el-step :title="t('restore.step2Title')" />
      </el-steps>

      <div class="step-content">
        <template v-if="currentStep === 0">
          <el-form
            ref="formRef"
            :model="restoreForm"
            :rules="formRules"
            label-position="top"
            class="restore-form"
            @submit.prevent
          >
            <el-form-item :label="t('restore.storageType')" prop="storageType">
              <el-select v-model="restoreForm.storageType" style="width: 100%">
                <el-option label="WebDAV" value="webdav" />
              </el-select>
            </el-form-item>

            <el-form-item :label="t('restore.restoreWebdavAddress')" prop="webdavAddress">
              <el-input v-model="restoreForm.webdavAddress" :placeholder="t('restore.restoreWebdavAddressPlaceholder')" />
            </el-form-item>

            <el-form-item :label="t('restore.restoreWebdavUsername')" prop="webdavUsername">
              <el-input v-model="restoreForm.webdavUsername" :placeholder="t('restore.restoreWebdavUsernamePlaceholder')" />
            </el-form-item>

            <el-form-item :label="t('restore.restoreWebdavPassword')" prop="webdavPassword">
              <el-input v-model="restoreForm.webdavPassword" type="password" show-password :placeholder="t('restore.restoreWebdavPasswordPlaceholder')" />
            </el-form-item>

            <el-form-item :label="t('restore.restoreWebdavPath')" prop="webdavPath">
              <el-input v-model="restoreForm.webdavPath" :placeholder="t('restore.restoreWebdavPathPlaceholder')" />
            </el-form-item>

            <el-form-item :label="t('restore.encrypted')">
              <el-switch v-model="restoreForm.encrypted" />
            </el-form-item>

            <el-form-item v-if="restoreForm.encrypted" :label="t('restore.backupPassword')" prop="backupPassword">
              <el-input v-model="restoreForm.backupPassword" type="password" show-password :placeholder="t('restore.backupPasswordPlaceholder')" />
            </el-form-item>

            <el-form-item :label="t('restore.localPath')" prop="localPath">
              <FolderSelect v-model="restoreForm.localPath" :placeholder="t('restore.localPathPlaceholder')" />
            </el-form-item>
          </el-form>
        </template>

        <template v-else-if="currentStep === 1">
          <div class="restore-progress">
            <div class="progress-header">
              <div class="progress-info">
                <span class="progress-label">{{ t('restore.restoreProgress') }}</span>
                <span class="progress-count">{{ t('restore.fileCount', { current: progress.success_count, total: progress.total_count }) }}</span>
                <span v-if="progress.pending_count > 0" class="pending-count">{{ t('restore.pendingCount', { count: progress.pending_count }) }}</span>
              </div>
            </div>

            <div class="table-wrapper">
              <el-table :data="displayItems" height="100%">
                <el-table-column prop="name" :label="t('restore.fileName')" min-width="200" show-overflow-tooltip />
                <el-table-column :label="t('restore.progress')" width="150">
                  <template #default="{ row }">
                    <el-progress
                      :percentage="getProgressPercentage(row)"
                      :stroke-width="8"
                      :status="row.status === 'failed' ? 'exception' : undefined"
                    />
                  </template>
                </el-table-column>
                <el-table-column prop="status" :label="t('restore.status')" width="100">
                  <template #default="{ row }">
                    <div>
                      <span :class="['status-tag', `status-${row.status}`]">
                        {{ getStatusText(row.status) }}
                      </span>
                      <span v-if="row.error" class="error-reason" :title="row.error">{{ row.error }}</span>
                    </div>
                  </template>
                </el-table-column>
                <el-table-column :label="t('restore.actions')" width="80">
                  <template #default="{ row }">
                    <el-button
                      v-if="row.status === 'failed'"
                      type="primary"
                      size="small"
                      link
                      @click="handleRetryFile(row.name)"
                      :loading="retryingFiles.has(row.name)"
                    >
                      {{ t('restore.retry') }}
                    </el-button>
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </div>
        </template>
      </div>
    </div>

    <template #footer>
      <div class="drawer-footer">
        <template v-if="currentStep === 0">
          <el-button @click="handleClose">{{ t('common.cancel') }}</el-button>
          <el-button type="primary" @click="handleNextStep" :loading="starting">{{ t('restore.nextStep') }}</el-button>
        </template>
        <template v-else>
          <el-button @click="handleClose">{{ t('common.cancel') }}</el-button>
          <el-button v-if="progress.is_running" type="danger" @click="handleCancelRestore">
            {{ t('restore.cancelRestore') }}
          </el-button>
        </template>
      </div>
    </template>
  </el-drawer>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onUnmounted, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import FolderSelect from '@/components/FolderSelect.vue'
import {
  checkRestoreTarget,
  startRestore,
  getRestoreProgress,
  cancelRestore,
  retryRestoreFile,
  type RestorePendingItem,
  type RestoreProgressResponse
} from '@/api/system'

const { t } = useI18n()

const visible = ref(false)
const currentStep = ref(0)
const formRef = ref<FormInstance>()
const starting = ref(false)
const restoreCompleted = ref(false)

const isMobile = ref(window.innerWidth <= 768)

const taskId = ref('')
const retryingFiles = ref(new Set<string>())

const progress = ref<RestoreProgressResponse>({
  is_running: false,
  downloading_items: [],
  failed_items: [],
  pending_count: 0,
  total_count: 0,
  success_count: 0,
  downloaded_bytes: 0
})

const displayItems = computed(() => {
  const downloading = progress.value.downloading_items.slice(0, 100)
  const failed = progress.value.failed_items
  return [...downloading, ...failed]
})

let progressTimer: ReturnType<typeof setInterval> | null = null

const restoreForm = reactive({
  storageType: 'webdav',
  webdavAddress: '',
  webdavUsername: '',
  webdavPassword: '',
  webdavPath: '',
  encrypted: false,
  backupPassword: '',
  localPath: '',
})

const validateBackupPassword = (_rule: unknown, value: string, callback: (error?: Error) => void) => {
  if (restoreForm.encrypted && !value) {
    callback(new Error(t('restore.backupPasswordRequired')))
  } else {
    callback()
  }
}

const formRules = computed<FormRules>(() => ({
  storageType: [{ required: true, message: t('restore.storageTypeRequired'), trigger: 'change' }],
  webdavAddress: [{ required: true, message: t('restore.restoreWebdavAddressRequired'), trigger: 'blur' }],
  webdavUsername: [{ required: true, message: t('restore.restoreWebdavUsernameRequired'), trigger: 'blur' }],
  webdavPassword: [{ required: true, message: t('restore.restoreWebdavPasswordRequired'), trigger: 'blur' }],
  webdavPath: [{ required: true, message: t('restore.restoreWebdavPathRequired'), trigger: 'blur' }],
  backupPassword: [{ validator: validateBackupPassword, trigger: 'blur' }],
  localPath: [{ required: true, message: t('restore.localPathRequired'), trigger: 'blur' }],
}))

const getProgressPercentage = (row: RestorePendingItem) => {
  if (row.total_bytes === 0) return 0
  return Math.round((row.downloaded_bytes / row.total_bytes) * 100)
}

const getStatusText = (status: string) => {
  const texts: Record<string, string> = {
    pending: t('restore.statusPending'),
    downloading: t('restore.statusDownloading'),
    retrying: t('restore.statusRetrying'),
    completed: t('restore.statusCompleted'),
    failed: t('restore.statusFailed'),
  }
  return texts[status] || status
}

let progressNotified = false

const startProgressPolling = () => {
  stopProgressPolling()
  progressNotified = false
  progressTimer = setInterval(async () => {
    if (!taskId.value) return

    try {
      const result = await getRestoreProgress(taskId.value)
      progress.value = result

      if (!result.is_running && !progressNotified) {
        progressNotified = true
        if (result.failed_items.length === 0) {
          restoreCompleted.value = true
          ElMessage.success({ __key: 'restore.restoreSuccess' })
          stopProgressPolling()
        } else {
          ElMessage.warning({ __key: 'restore.restorePartialSuccess', __params: { count: result.failed_items.length } })
        }
      }
    } catch (error) {
      console.error('Failed to get restore progress:', error)
    }
  }, 1000)
}

const stopProgressPolling = () => {
  if (progressTimer) {
    clearInterval(progressTimer)
    progressTimer = null
  }
}

const open = () => {
  currentStep.value = 0
  resetForm()
  visible.value = true
}

const resetForm = () => {
  restoreForm.storageType = 'webdav'
  restoreForm.webdavAddress = ''
  restoreForm.webdavUsername = ''
  restoreForm.webdavPassword = ''
  restoreForm.webdavPath = ''
  restoreForm.encrypted = false
  restoreForm.backupPassword = ''
  restoreForm.localPath = ''
  formRef.value?.clearValidate()
  taskId.value = ''
  restoreCompleted.value = false
  progress.value = {
    is_running: false,
    downloading_items: [],
    failed_items: [],
    pending_count: 0,
    total_count: 0,
    success_count: 0,
    downloaded_bytes: 0
  }
}

const handleNextStep = async () => {
  const valid = await formRef.value?.validate()
  if (!valid) return

  // 检查目标目录是否为空
  try {
    const checkResult = await checkRestoreTarget(restoreForm.localPath)
    if (!checkResult.is_empty) {
      // 目录不为空，显示警告
      const fileList = checkResult.files.length > 5
        ? [...checkResult.files.slice(0, 5), `...(${checkResult.file_count - 5} more)`]
        : checkResult.files

      try {
        await ElMessageBox.confirm(
          t('restore.directoryNotEmptyWarning', {
            count: checkResult.file_count,
            files: fileList.join(', ')
          }),
          t('restore.warning'),
          {
            confirmButtonText: t('restore.continueRestore'),
            cancelButtonText: t('common.cancel'),
            type: 'warning',
          }
        )
      } catch {
        // 用户取消
        return
      }
    }
  } catch (error) {
    console.error('Failed to check target directory:', error)
  }

  // 启动恢复任务
  starting.value = true
  try {
    const result = await startRestore({
      storage_type: restoreForm.storageType,
      storage_config: {
        address: restoreForm.webdavAddress,
        username: restoreForm.webdavUsername,
        password: restoreForm.webdavPassword,
        path: restoreForm.webdavPath,
      },
      encrypted: restoreForm.encrypted,
      backup_password: restoreForm.encrypted ? restoreForm.backupPassword : undefined,
      local_path: restoreForm.localPath,
    })

    taskId.value = result.task_id!
    currentStep.value = 1
    startProgressPolling()
  } catch (error: any) {
    const code = error.message
    let msg = ''
    if (code === 'DECRYPTION_FAILED') {
      msg = t('restore.decryptionFailed')
    } else if (code === 'INDEX_NOT_FOUND') {
      msg = t('restore.indexNotFound')
    } else if (code === 'STORAGE_CONNECTION_ERROR') {
      msg = t('restore.storageConnectionError')
    } else {
      msg = t('restore.startRestoreFailed')
    }
    if (error.detailMessage) {
      msg += '\n' + error.detailMessage
    }
    ElMessage.error(msg)
  } finally {
    starting.value = false
  }
}

const handleCancelRestore = async () => {
  try {
    await cancelRestore(taskId.value)
    // 取消请求发送后，等待任务实际停止（通过轮询检测 is_running: false）
  } catch (error) {
    console.error('Failed to cancel restore:', error)
  }
}

const handleRetryFile = async (filePath: string) => {
  if (retryingFiles.value.has(filePath)) return

  retryingFiles.value.add(filePath)
  try {
    await retryRestoreFile(taskId.value, filePath)
    ElMessage.success({ __key: 'restore.retryStarted' })
  } catch (error: any) {
    if (error.message === 'ALREADY_RETRYING') {
      ElMessage.warning({ __key: 'restore.alreadyRetrying' })
    } else if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'restore.retryFailed' })
    }
  } finally {
    retryingFiles.value.delete(filePath)
  }
}

// 关闭前的钩子：阻止未完成时关闭
const handleBeforeClose = async (done: (cancel: boolean) => void) => {
  if (currentStep.value === 0) {
    done(false)
    visible.value = false
    return
  }

  if (progress.value.is_running) {
    try {
      await ElMessageBox.confirm(
        t('restore.cancelRestoreConfirm'),
        t('restore.warning'),
        {
          confirmButtonText: t('common.confirm'),
          cancelButtonText: t('common.cancel'),
          type: 'warning',
        }
      )
      await cancelRestore(taskId.value)
    } catch {
      done(true)
      return
    }
  }

  stopProgressPolling()
  done(false)
  visible.value = false
}

const handleClose = () => {
  stopProgressPolling()
  visible.value = false
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
.restore-drawer :deep(.el-drawer__body) {
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

.restore-steps {
  flex-shrink: 0;
  margin-bottom: 24px;
}

.step-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.restore-form {
  max-width: 100%;
}

.restore-progress {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.progress-header {
  flex-shrink: 0;
  margin-bottom: 16px;
}

.progress-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.progress-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.progress-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.pending-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.table-wrapper {
  flex: 1;
  min-height: 0;
}

.status-tag {
  font-size: 12px;
  font-weight: 500;
}

.status-tag.status-pending {
  color: var(--el-text-color-secondary);
}

.status-tag.status-downloading {
  color: var(--el-color-primary);
}

.status-tag.status-retrying {
  color: var(--el-color-warning);
}

.status-tag.status-completed {
  color: var(--el-color-success);
}

.status-tag.status-failed {
  color: var(--el-color-danger);
}

.error-reason {
  display: block;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-footer {
  display: flex;
  justify-content: flex-end;
}
</style>
