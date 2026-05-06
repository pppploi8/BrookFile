<template>
  <div class="profile-container">
    <div class="profile-card">
      <el-tabs v-model="activeTab" class="profile-tabs" style="height: 100%">
        <el-tab-pane :label="t('profile.basicInfo')" name="basic">
          <div class="tab-content">
            <div class="basic-info-wrapper">
              <div class="form-section">
                <div class="form-item">
                  <label class="form-label">{{ t('profile.username') }}</label>
                  <el-input
                    v-model="userInfo.username"
                    disabled
                    class="form-input"
                  />
                </div>

                <div class="form-item">
                  <label class="form-label">{{ t('profile.avatar') }}</label>
                  <div class="avatar-section">
                    <el-avatar :size="80" class="avatar-preview" :src="avatarUrl">
                      <template v-if="!avatarUrl">
                        {{ userInfo.username?.charAt(0).toUpperCase() }}
                      </template>
                    </el-avatar>
                    <input
                      ref="fileInput"
                      type="file"
                      accept="image/jpeg,image/png,image/gif,image/webp"
                      style="display: none"
                      @change="handleFileSelect"
                    />
                    <div class="avatar-buttons">
                      <el-button type="primary" plain @click="triggerFileSelect">
                        {{ t('profile.selectAvatar') }}
                      </el-button>
                      <el-button type="danger" plain @click="confirmDeleteAvatar" v-if="avatarUrl">
                        {{ t('profile.clearAvatar') }}
                      </el-button>
                    </div>
                  </div>
                </div>

                <div class="form-item">
                  <label class="form-label">{{ t('profile.changePassword') }}</label>
                  <div class="password-form">
                    <el-input
                      v-model="passwordForm.currentPassword"
                      type="password"
                      :placeholder="t('profile.currentPasswordPlaceholder')"
                      show-password
                      class="form-input"
                    />
                    <el-input
                      v-model="passwordForm.newPassword"
                      type="password"
                      :placeholder="t('profile.newPasswordPlaceholder')"
                      show-password
                      class="form-input"
                    />
                    <el-input
                      v-model="passwordForm.confirmPassword"
                      type="password"
                      :placeholder="t('profile.confirmPasswordPlaceholder')"
                      show-password
                      class="form-input"
                    />
                    <el-button type="primary" @click="handleChangePassword">
                      {{ t('profile.changePassword') }}
                    </el-button>
                  </div>
                </div>


              </div>
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane :label="t('profile.webdavConfig')" name="webdav" style="height: 100%">
          <div class="tab-content backup-content">
            <div class="backup-toolbar">
              <el-button type="primary" @click="handleAddWebDav">
                <el-icon><Plus /></el-icon>
                {{ t('profile.webdavAddConfig') }}
              </el-button>
            </div>
            <div class="backup-table-wrapper">
              <el-table :data="webdavList" class="backup-table" show-overflow-tooltip v-loading="loadingWebdavList">
                <el-table-column prop="dav_path" :label="t('profile.webdavPath')" min-width="120">
                  <template #default="{ row }">
                    {{ row.global_access ? t('profile.webdavGlobalAccess') : row.dav_path }}
                  </template>
                </el-table-column>
                <el-table-column prop="access_path" :label="t('profile.webdavAccessPath')" min-width="150" />
                <el-table-column prop="url" :label="t('profile.webdavUrl')" min-width="150" />
                <el-table-column :label="t('profile.webdavPermission')" min-width="120">
                  <template #default="{ row }">
                    {{ getPermissionText(row.permission) }}
                  </template>
                </el-table-column>
                <el-table-column :label="t('profile.backupOperations')" :min-width="isMobile ? 80 : 150" fixed="right">
                  <template #default="{ row }">
                    <el-link type="primary" :icon="Edit" @click="handleEditWebDav(row)">
                      <span v-if="!isMobile">{{ t('profile.edit') }}</span>
                    </el-link>
                    <el-link type="danger" :icon="Delete" @click="handleDeleteWebDav(row)">
                      <span v-if="!isMobile">{{ t('profile.delete') }}</span>
                    </el-link>
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane :label="t('profile.featureOrder')" name="feature">
          <div class="tab-content">
            <div class="feature-order-wrapper">
              <p class="feature-order-tip">{{ t('profile.featureOrderTip') }}</p>
              <div class="feature-order-list">
                <div
                  v-for="(item, index) in featureItems"
                  :key="item.key"
                  class="feature-item"
                  draggable="true"
                  @dragstart="handleDragStart(index, $event)"
                  @dragover.prevent
                  @drop="handleDrop(index, $event)"
                  @dragend="handleDragEnd"
                  :class="{ dragging: dragIndex === index }"
                >
                  <el-icon class="drag-handle">
                    <svg viewBox="0 0 24 24" fill="currentColor">
                      <path d="M8 6a2 2 0 1 1-4 0 2 2 0 0 1 4 0zM8 12a2 2 0 1 1-4 0 2 2 0 0 1 4 0zM8 18a2 2 0 1 1-4 0 2 2 0 0 1 4 0zM14 6a2 2 0 1 1-4 0 2 2 0 0 1 4 0zM14 12a2 2 0 1 1-4 0 2 2 0 0 1 4 0zM14 18a2 2 0 1 1-4 0 2 2 0 0 1 4 0z"/>
                    </svg>
                  </el-icon>
                  <el-icon class="feature-icon">
                    <component :is="item.icon" />
                  </el-icon>
                  <span class="feature-label">{{ item.label }}</span>
                </div>
              </div>
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane :label="t('profile.cloudBackup')" name="backup" style="height: 100%">
          <div class="tab-content backup-content">
            <div class="backup-toolbar">
              <el-button type="primary" @click="handleAddBackup">
                <el-icon><Plus /></el-icon>
                {{ t('profile.add') }}
              </el-button>
              <el-button @click="handleRestoreBackup">
                <el-icon><Refresh /></el-icon>
                {{ t('profile.restoreBackup') }}
              </el-button>
            </div>
            <div class="backup-table-wrapper">
              <el-table :data="backupList" class="backup-table" show-overflow-tooltip v-loading="loadingBackupList">
                <el-table-column prop="name" :label="t('profile.backupName')" min-width="120" />
                <el-table-column prop="local_path" :label="t('profile.backupPath')" min-width="150" />
                <el-table-column :label="t('profile.storageType')" min-width="100">
                  <template #default="{ row }">
                    {{ row.storage_type === 'webdav' ? 'WebDAV' : row.storage_type }}
                  </template>
                </el-table-column>
                <el-table-column :label="t('profile.backupCycle')" min-width="100">
                  <template #default="{ row }">
                    {{ getCycleText(row.cycle) }}
                  </template>
                </el-table-column>
                <el-table-column :label="t('profile.backupStatus')" min-width="100">
                  <template #default="{ row }">
                    <span :class="['backup-status', `status-${row.status}`]">
                      {{ getStatusText(row.status) }}
                    </span>
                  </template>
                </el-table-column>
                <el-table-column :label="t('profile.backupNextTime')" min-width="180">
                  <template #default="{ row }">
                    {{ row.next_backup_time || '-' }}
                  </template>
                </el-table-column>
                <el-table-column :label="t('profile.backupLastTime')" min-width="180">
                  <template #default="{ row }">
                    {{ row.last_backup_time || '-' }}
                  </template>
                </el-table-column>
                <el-table-column :label="t('profile.backupOperations')" :min-width="isMobile ? 110 : 200" fixed="right">
                  <template #default="{ row }">
                    <el-link type="primary" :icon="Edit" @click="handleEditBackup(row)">
                      <span v-if="!isMobile">{{ t('profile.edit') }}</span>
                    </el-link>
                    <el-link type="primary" :icon="Document" @click="handleViewLog(row)">
                      <span v-if="!isMobile">{{ t('profile.log') }}</span>
                    </el-link>
                    <el-link type="danger" :icon="Delete" @click="handleDeleteBackup(row)">
                      <span v-if="!isMobile">{{ t('profile.delete') }}</span>
                    </el-link>
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </div>

    <el-drawer
      v-model="drawerVisible"
      :title="isEdit ? t('profile.editBackup') : t('profile.addBackup')"
      direction="rtl"
      :size="isMobile ? '100%' : '450px'"
      destroy-on-close
    >
      <el-form
        ref="formRef"
        :model="backupForm"
        :rules="formRules"
        label-position="top"
        @submit.prevent
      >
        <el-form-item :label="t('profile.backupName')" prop="name">
          <el-input v-model="backupForm.name" :placeholder="t('profile.backupNamePlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.storageType')" prop="type">
          <el-select v-model="backupForm.type" style="width: 100%">
            <el-option label="WebDAV" value="webdav" />
          </el-select>
        </el-form-item>

        <el-form-item :label="t('profile.backupWebdavAddress')" prop="webdavAddress">
          <el-input v-model="backupForm.webdavAddress" :placeholder="t('profile.backupWebdavAddressPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.backupWebdavUsername')" prop="webdavUsername">
          <el-input v-model="backupForm.webdavUsername" :placeholder="t('profile.backupWebdavUsernamePlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.backupWebdavPassword')" prop="webdavPassword">
          <el-input v-model="backupForm.webdavPassword" type="password" show-password :placeholder="isEdit ? t('profile.backupWebdavPasswordEditPlaceholder') : t('profile.backupWebdavPasswordPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.backupWebdavPath')" prop="webdavPath">
          <el-input v-model="backupForm.webdavPath" :placeholder="t('profile.backupWebdavPathPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.encrypted')" prop="encrypted">
          <el-switch v-model="backupForm.encrypted" />
        </el-form-item>

        <el-form-item v-if="backupForm.encrypted" :label="t('profile.backupPassword')" prop="backupPassword">
          <el-input v-model="backupForm.backupPassword" type="password" show-password :placeholder="isEdit ? t('profile.backupPasswordEditPlaceholder') : t('profile.backupPasswordPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.backupPath')" prop="path">
          <FolderSelect v-model="backupForm.path" :placeholder="t('profile.backupPathPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.backupCycle')" prop="cycle">
          <el-select v-model="backupForm.cycle" style="width: 100%" @change="handleCycleChange">
            <el-option :label="t('profile.cycleDaily')" value="daily" />
            <el-option :label="t('profile.cycleWeekly')" value="weekly" />
            <el-option :label="t('profile.cycleMonthly')" value="monthly" />
            <el-option :label="t('profile.cycleYearly')" value="yearly" />
          </el-select>
        </el-form-item>

        <el-form-item v-if="backupForm.cycle === 'daily'" :label="t('profile.backupTime')" prop="backupTime">
          <el-time-picker
            v-model="backupForm.backupTime"
            format="HH:mm"
            value-format="HH:mm"
            style="width: 100%"
          />
        </el-form-item>

        <template v-else-if="backupForm.cycle === 'weekly'">
          <el-form-item :label="t('profile.backupWeekDay')" prop="backupWeekDay">
            <el-select v-model="backupForm.backupWeekDay" style="width: 100%">
              <el-option v-for="day in weekDays" :key="day.value" :label="day.label" :value="day.value" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('profile.backupTime')" prop="backupTime">
            <el-time-picker
              v-model="backupForm.backupTime"
              format="HH:mm"
              value-format="HH:mm"
              style="width: 100%"
            />
          </el-form-item>
        </template>

        <template v-else-if="backupForm.cycle === 'monthly'">
          <el-form-item :label="t('profile.backupMonthDay')" prop="backupMonthDay">
            <el-select v-model="backupForm.backupMonthDay" style="width: 100%">
              <el-option v-for="day in 31" :key="day" :label="t('profile.dayN', { n: day })" :value="String(day)" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('profile.backupTime')" prop="backupTime">
            <el-time-picker
              v-model="backupForm.backupTime"
              format="HH:mm"
              value-format="HH:mm"
              style="width: 100%"
            />
          </el-form-item>
        </template>

        <template v-else-if="backupForm.cycle === 'yearly'">
          <el-form-item :label="t('profile.backupYearDate')" prop="backupYearDate">
            <el-date-picker
              v-model="backupForm.backupYearDate"
              type="date"
              format="MM-DD"
              value-format="MM-DD"
              style="width: 100%"
            />
          </el-form-item>
          <el-form-item :label="t('profile.backupTime')" prop="backupTime">
            <el-time-picker
              v-model="backupForm.backupTime"
              format="HH:mm"
              value-format="HH:mm"
              style="width: 100%"
            />
          </el-form-item>
        </template>
      </el-form>

      <template #footer>
        <el-button @click="drawerVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="backupSaving" @click="handleSaveBackup">{{ t('common.submit') }}</el-button>
      </template>
    </el-drawer>

    <BackupLogDrawer ref="backupLogDrawerRef" />
    <RestoreDrawer ref="restoreDrawerRef" />

    <el-drawer
      v-model="webdavDrawerVisible"
      :title="isEditWebDav ? t('profile.webdavEditConfig') : t('profile.webdavAddConfig')"
      direction="rtl"
      :size="isMobile ? '100%' : '450px'"
      destroy-on-close
    >
      <el-form
        ref="webdavFormRef"
        :model="webdavForm"
        :rules="webdavFormRules"
        label-position="top"
        @submit.prevent
      >
        <el-form-item :label="t('profile.webdavGlobalAccess')" prop="global_access">
          <el-switch v-model="webdavForm.global_access" />
          <span class="form-tip" style="margin-left: 12px; color: var(--el-text-color-secondary); font-size: 12px;">{{ t('profile.webdavGlobalAccessTip') }}</span>
        </el-form-item>

        <el-form-item v-if="!webdavForm.global_access" :label="t('profile.webdavPath')" prop="dav_path">
          <el-input v-model="webdavForm.dav_path" :placeholder="t('profile.webdavPathPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.webdavAccessPath')" prop="access_path">
          <FolderSelect v-model="webdavForm.access_path" :placeholder="t('profile.webdavAccessPathPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.webdavPassword')" prop="password">
          <el-input v-model="webdavForm.password" type="password" show-password :placeholder="isEditWebDav ? t('profile.webdavPasswordEditPlaceholder') : t('profile.webdavPasswordPlaceholder')" />
        </el-form-item>

        <el-form-item :label="t('profile.webdavPermission')" prop="permission">
          <el-select v-model="webdavForm.permission" style="width: 100%">
            <el-option :label="t('profile.webdavPermissionFullControl') + ' - ' + t('profile.webdavPermissionFullControlDesc')" value="full_control" />
            <el-option :label="t('profile.webdavPermissionEdit') + ' - ' + t('profile.webdavPermissionEditDesc')" value="edit" />
            <el-option :label="t('profile.webdavPermissionReadOnly') + ' - ' + t('profile.webdavPermissionReadOnlyDesc')" value="read_only" />
          </el-select>
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="webdavDrawerVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSaveWebDav">{{ t('common.submit') }}</el-button>
      </template>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox, type FormInstance, type FormRules } from 'element-plus'
import { Plus, Refresh, Edit, Document, Delete, EditPen, Key } from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import BackupLogDrawer from '@/components/BackupLogDrawer.vue'
import RestoreDrawer from '@/components/RestoreDrawer.vue'
import FolderSelect from '@/components/FolderSelect.vue'
import { uploadAvatar, fetchAvatar, deleteAvatar, changePassword, listBackupRules, getBackupRule, createBackupRule, updateBackupRule, deleteBackupRule, updateFeatureOrder, listWebDavConfigs, createWebDavConfig, updateWebDavConfig, deleteWebDavConfig } from '@/api/system'
import router from '@/router'

const { t } = useI18n()
const userStore = useUserStore()

const activeTab = ref('basic')
const fileInput = ref<HTMLInputElement | null>(null)
const formRef = ref<FormInstance>()
const drawerVisible = ref(false)
const isEdit = ref(false)
const backupSaving = ref(false)
const backupLogDrawerRef = ref<InstanceType<typeof BackupLogDrawer> | null>(null)
const restoreDrawerRef = ref<InstanceType<typeof RestoreDrawer> | null>(null)

// 移动端检测
const isMobile = ref(window.innerWidth <= 768)
const handleResize = () => {
  isMobile.value = window.innerWidth <= 768
}

// 功能排序相关
const allFeatures = [
  { key: 'file', label: computed(() => t('home.categories.files')), icon: Document },
  { key: 'note', label: computed(() => t('home.categories.notes')), icon: EditPen },
  { key: 'password', label: computed(() => t('home.categories.passwords')), icon: Key },
]

const featureOrder = ref<string[]>([])
const dragIndex = ref<number | null>(null)

const featureItems = computed(() => {
  return featureOrder.value.map(key => {
    const item = allFeatures.find(f => f.key === key)
    return item ? { ...item, label: item.label.value } : null
  }).filter((item): item is NonNullable<typeof item> => item !== null)
})

const initFeatureOrder = () => {
  const savedOrder = userStore.user?.feature_order
  const validKeys = allFeatures.map(f => f.key)
  if (savedOrder) {
    const savedKeys = savedOrder.split(',').filter(key => validKeys.includes(key))
    validKeys.forEach(key => {
      if (!savedKeys.includes(key)) {
        savedKeys.push(key)
      }
    })
    featureOrder.value = savedKeys
  } else {
    featureOrder.value = allFeatures.map(f => f.key)
  }
}

const handleDragStart = (index: number, event: DragEvent) => {
  dragIndex.value = index
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
  }
}

const handleDrop = async (index: number, event: DragEvent) => {
  event.preventDefault()
  if (dragIndex.value === null || dragIndex.value === index) return

  const items = [...featureOrder.value]
  const removed = items.splice(dragIndex.value, 1)[0]
  if (!removed) return
  items.splice(index, 0, removed)
  featureOrder.value = items

  // 保存到后端
  const orderString = items.join(',')
  try {
    const result = await updateFeatureOrder(orderString)
    if (result.success) {
      userStore.setFeatureOrder(orderString)
      ElMessage.success({ __key: 'profile.featureOrderSaved' })
    }
  } catch {
    // 如果保存失败，恢复原顺序
    initFeatureOrder()
  }
}

const handleDragEnd = () => {
  dragIndex.value = null
}

const weekDays = computed(() => [
  { value: '1', label: t('profile.weekMonday') },
  { value: '2', label: t('profile.weekTuesday') },
  { value: '3', label: t('profile.weekWednesday') },
  { value: '4', label: t('profile.weekThursday') },
  { value: '5', label: t('profile.weekFriday') },
  { value: '6', label: t('profile.weekSaturday') },
  { value: '7', label: t('profile.weekSunday') },
])

const userInfo = reactive({
  username: userStore.user?.username || '',
})

const avatarUrl = ref<string | undefined>(undefined)

watch(() => userStore.avatarBlob, (newBlob) => {
  if (avatarUrl.value) {
    URL.revokeObjectURL(avatarUrl.value)
    avatarUrl.value = undefined
  }
  if (newBlob) {
    avatarUrl.value = URL.createObjectURL(newBlob)
  }
}, { immediate: true })

const passwordForm = reactive({
  currentPassword: '',
  newPassword: '',
  confirmPassword: '',
})

const backupForm = reactive({
  id: '',
  name: '',
  type: 'webdav',
  path: '',
  webdavAddress: '',
  webdavUsername: '',
  webdavPassword: '',
  webdavPath: '',
  encrypted: false,
  backupPassword: '',
  cycle: 'daily',
  backupTime: '',
  backupWeekDay: '1',
  backupMonthDay: '1',
  backupYearDate: '',
})

const handleCycleChange = () => {
  backupForm.backupTime = ''
  backupForm.backupWeekDay = ''
  backupForm.backupMonthDay = ''
  backupForm.backupYearDate = ''
}

const formRules = computed<FormRules>(() => {
  const rules: Record<string, any[]> = {
    name: [{ required: true, message: t('profile.backupNameRequired'), trigger: 'blur' }],
    type: [{ required: true, message: t('profile.storageTypeRequired'), trigger: 'change' }],
    webdavAddress: [{ required: true, message: t('profile.backupWebdavAddressRequired'), trigger: 'blur' }],
    webdavUsername: [{ required: true, message: t('profile.backupWebdavUsernameRequired'), trigger: 'blur' }],
    webdavPassword: [{ required: !isEdit.value, message: t('profile.backupWebdavPasswordRequired'), trigger: 'blur' }],
    webdavPath: [{ required: true, message: t('profile.backupWebdavPathRequired'), trigger: 'blur' }],
    path: [{ required: true, message: t('profile.backupPathRequired'), trigger: 'blur' }],
    cycle: [{ required: true, message: t('profile.backupCycleRequired'), trigger: 'change' }],
    backupTime: [{ required: true, message: t('profile.backupTimeRequired'), trigger: 'change' }],
  }
  if (backupForm.cycle === 'weekly') {
    rules.backupWeekDay = [{ required: true, message: t('profile.backupWeekDayRequired'), trigger: 'change' }]
  } else if (backupForm.cycle === 'monthly') {
    rules.backupMonthDay = [{ required: true, message: t('profile.backupMonthDayRequired'), trigger: 'change' }]
  } else if (backupForm.cycle === 'yearly') {
    rules.backupYearDate = [{ required: true, message: t('profile.backupYearDateRequired'), trigger: 'change' }]
  }
  return rules
})

interface BackupItem {
  id: string
  storage_type: string
  name: string
  local_path: string
  cycle: string
  backup_time: Record<string, unknown>
  status: string
  next_backup_time: string | null
  last_backup_time: string | null
  created_at: string | null
}

const backupList = ref<BackupItem[]>([])

const loadingBackupList = ref(false)

const loadBackupList = async () => {
  loadingBackupList.value = true
  try {
    const data = await listBackupRules()
    backupList.value = data
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'profile.loadBackupFailed' })
    }
  } finally {
    loadingBackupList.value = false
  }
}

const getStatusText = (status: string) => {
  const texts: Record<string, string> = {
    success: t('profile.backupSuccess'),
    failed: t('profile.backupFailed'),
    idle: t('profile.backupWaiting'),
    running: t('profile.backupRunning'),
  }
  return texts[status] || status
}

const getCycleText = (cycle: string) => {
  const texts: Record<string, string> = {
    daily: t('profile.cycleDaily'),
    weekly: t('profile.cycleWeekly'),
    monthly: t('profile.cycleMonthly'),
    yearly: t('profile.cycleYearly'),
  }
  return texts[cycle] || cycle
}

const handleAddBackup = () => {
  isEdit.value = false
  resetBackupForm()
  drawerVisible.value = true
}

const handleRestoreBackup = () => {
  restoreDrawerRef.value?.open()
}

const resetBackupForm = () => {
  backupForm.id = ''
  backupForm.name = ''
  backupForm.type = 'webdav'
  backupForm.path = ''
  backupForm.webdavAddress = ''
  backupForm.webdavUsername = ''
  backupForm.webdavPassword = ''
  backupForm.webdavPath = ''
  backupForm.encrypted = false
  backupForm.backupPassword = ''
  backupForm.cycle = 'daily'
  backupForm.backupTime = ''
  backupForm.backupWeekDay = '1'
  backupForm.backupMonthDay = '1'
  backupForm.backupYearDate = ''
  formRef.value?.clearValidate()
}

const handleEditBackup = async (row: BackupItem) => {
  isEdit.value = true
  try {
    const detail = await getBackupRule(row.id)
    backupForm.id = detail.id
    backupForm.name = detail.name
    backupForm.type = detail.storage_type
    backupForm.path = detail.local_path
    backupForm.webdavAddress = detail.storage_config.address
    backupForm.webdavUsername = detail.storage_config.username
    backupForm.webdavPassword = ''
    backupForm.webdavPath = detail.storage_config.path
    backupForm.encrypted = detail.encrypted
    backupForm.backupPassword = ''
    backupForm.cycle = detail.cycle
    
    if (detail.cycle === 'daily') {
      backupForm.backupTime = detail.backup_time.time as string || ''
    } else if (detail.cycle === 'weekly') {
      backupForm.backupWeekDay = String(detail.backup_time.week_day || 1)
      backupForm.backupTime = detail.backup_time.time as string || ''
    } else if (detail.cycle === 'monthly') {
      backupForm.backupMonthDay = String(detail.backup_time.month_day || 1)
      backupForm.backupTime = detail.backup_time.time as string || ''
    } else if (detail.cycle === 'yearly') {
      backupForm.backupYearDate = detail.backup_time.year_date as string || ''
      backupForm.backupTime = detail.backup_time.time as string || ''
    }
    
    formRef.value?.clearValidate()
    drawerVisible.value = true
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'profile.loadBackupFailed' })
    }
  }
}

const handleSaveBackup = async () => {
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return

  if (!isEdit.value && backupForm.encrypted && !backupForm.backupPassword) {
    ElMessage.error({ __key: 'profile.backupPasswordRequired' })
    return
  }

  let backupTime: Record<string, unknown> = {}
  if (backupForm.cycle === 'daily') {
    backupTime = { time: backupForm.backupTime }
  } else if (backupForm.cycle === 'weekly') {
    backupTime = { week_day: parseInt(backupForm.backupWeekDay), time: backupForm.backupTime }
  } else if (backupForm.cycle === 'monthly') {
    backupTime = { month_day: parseInt(backupForm.backupMonthDay), time: backupForm.backupTime }
  } else if (backupForm.cycle === 'yearly') {
    backupTime = { year_date: backupForm.backupYearDate, time: backupForm.backupTime }
  }

  backupSaving.value = true
  try {
    if (isEdit.value) {
      const updateData = {
        id: backupForm.id,
        name: backupForm.name,
        storage_type: backupForm.type,
        storage_config: {
          address: backupForm.webdavAddress,
          username: backupForm.webdavUsername,
          password: backupForm.webdavPassword,
          path: backupForm.webdavPath,
        },
        local_path: backupForm.path,
        encrypted: backupForm.encrypted,
        backup_password: backupForm.backupPassword || undefined,
        cycle: backupForm.cycle,
        backup_time: backupTime,
      }
      await updateBackupRule(updateData)
      ElMessage.success({ __key: 'profile.editBackupSuccess' })
    } else {
      const createData = {
        name: backupForm.name,
        storage_type: backupForm.type,
        storage_config: {
          address: backupForm.webdavAddress,
          username: backupForm.webdavUsername,
          password: backupForm.webdavPassword,
          path: backupForm.webdavPath,
        },
        local_path: backupForm.path,
        encrypted: backupForm.encrypted,
        backup_password: backupForm.encrypted ? backupForm.backupPassword : undefined,
        cycle: backupForm.cycle,
        backup_time: backupTime,
      }
      await createBackupRule(createData)
      ElMessage.success({ __key: 'profile.addBackupSuccess' })
    }
    drawerVisible.value = false
    loadBackupList()
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'common.error' })
    }
  } finally {
    backupSaving.value = false
  }
}

const handleViewLog = (row: BackupItem) => {
  backupLogDrawerRef.value?.open(row.id)
}

const handleDeleteBackup = async (row: BackupItem) => {
  try {
    await ElMessageBox.confirm(
      t('profile.deleteBackupConfirm', { name: row.name }),
      t('common.confirm'),
      {
        type: 'warning',
      }
    )
    await deleteBackupRule(row.id)
    ElMessage.success({ __key: 'profile.deleteBackupSuccess' })
    loadBackupList()
  } catch {
  }
}

const handleChangePassword = async () => {
  if (!passwordForm.currentPassword || !passwordForm.newPassword || !passwordForm.confirmPassword) {
    ElMessage.error({ __key: 'profile.pleaseFillAllPasswordFields' })
    return
  }
  if (passwordForm.newPassword !== passwordForm.confirmPassword) {
    ElMessage.error({ __key: 'profile.passwordMismatch' })
    return
  }
  try {
    const result = await changePassword(passwordForm.currentPassword, passwordForm.newPassword)
    if (result.success) {
      ElMessage.success({ __key: 'profile.passwordChanged' })
      passwordForm.currentPassword = ''
      passwordForm.newPassword = ''
      passwordForm.confirmPassword = ''
      userStore.logout()
      router.push('/login')
    }
  } catch {
  }
}

const triggerFileSelect = () => {
  fileInput.value?.click()
}

const handleFileSelect = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  
  const allowedTypes = ['image/jpeg', 'image/png', 'image/gif', 'image/webp']
  if (!allowedTypes.includes(file.type)) {
    ElMessage.error({ __key: 'profile.invalidFileType' })
    return
  }
  
  try {
    const result = await uploadAvatar(file)
    if (result.success) {
      ElMessage.success({ __key: 'profile.avatarUploaded' })
      const blob = await fetchAvatar(userStore.user?.id || '')
      userStore.setAvatar(blob)
    }
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'profile.avatarUploadFailed' })
    }
  }
  
  target.value = ''
}

const confirmDeleteAvatar = async () => {
  try {
    await ElMessageBox.confirm(
      t('profile.deleteAvatarConfirm'),
      t('common.confirm'),
      {
        type: 'warning',
      }
    )
    await handleDeleteAvatar()
  } catch {
  }
}

const handleDeleteAvatar = async () => {
  try {
    const result = await deleteAvatar()
    if (result.success) {
      ElMessage.success({ __key: 'profile.avatarDeleted' })
      userStore.setAvatar(null)
    }
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'profile.avatarDeleteFailed' })
    }
  }
}

const webdavDrawerVisible = ref(false)
const isEditWebDav = ref(false)
const webdavFormRef = ref<FormInstance>()
const loadingWebdavList = ref(false)

interface WebDavItem {
  id: string
  dav_path: string
  access_path: string
  permission: string
  url: string
  global_access: boolean
  created_at: string
  updated_at: string
}

const webdavList = ref<WebDavItem[]>([])

const webdavForm = reactive({
  id: '',
  dav_path: '',
  access_path: '',
  password: '',
  permission: 'full_control',
  global_access: false,
})

const webdavFormRules = computed<FormRules>(() => ({
  dav_path: [{ required: !webdavForm.global_access, message: t('profile.webdavPathRequired'), trigger: 'blur' }],
  access_path: [{ required: !webdavForm.global_access, message: t('profile.webdavAccessPathRequired'), trigger: 'change' }],
  password: [{ required: !isEditWebDav.value, message: t('profile.webdavPasswordRequired'), trigger: 'blur' }],
  permission: [{ required: true, message: t('profile.webdavPermissionRequired'), trigger: 'change' }],
}))

const getPermissionText = (permission: string) => {
  const texts: Record<string, string> = {
    full_control: t('profile.webdavPermissionFullControl'),
    edit: t('profile.webdavPermissionEdit'),
    read_only: t('profile.webdavPermissionReadOnly'),
  }
  return texts[permission] || permission
}

const loadWebdavList = async () => {
  loadingWebdavList.value = true
  try {
    const data = await listWebDavConfigs()
    webdavList.value = data.configs || []
  } catch {
    ElMessage.error({ __key: 'common.error' })
  } finally {
    loadingWebdavList.value = false
  }
}

const resetWebdavForm = () => {
  webdavForm.id = ''
  webdavForm.dav_path = ''
  webdavForm.access_path = ''
  webdavForm.password = ''
  webdavForm.permission = 'full_control'
  webdavForm.global_access = false
  webdavFormRef.value?.clearValidate()
}

const handleAddWebDav = () => {
  isEditWebDav.value = false
  resetWebdavForm()
  webdavDrawerVisible.value = true
}

const handleEditWebDav = (row: WebDavItem) => {
  isEditWebDav.value = true
  webdavForm.id = row.id
  webdavForm.dav_path = row.dav_path
  webdavForm.access_path = row.access_path
  webdavForm.password = ''
  webdavForm.permission = row.permission
  webdavForm.global_access = row.global_access
  webdavFormRef.value?.clearValidate()
  webdavDrawerVisible.value = true
}

const handleSaveWebDav = async () => {
  const valid = await webdavFormRef.value?.validate().catch(() => false)
  if (!valid) return

  const submitDavPath = webdavForm.global_access ? '' : webdavForm.dav_path
  const submitAccessPath = webdavForm.global_access ? '' : (webdavForm.access_path === '/' ? '' : webdavForm.access_path)

  try {
    if (isEditWebDav.value) {
      await updateWebDavConfig({
        id: webdavForm.id,
        dav_path: submitDavPath,
        access_path: submitAccessPath,
        password: webdavForm.password || undefined,
        permission: webdavForm.permission,
        global_access: webdavForm.global_access,
      })
      ElMessage.success({ __key: 'profile.webdavEditSuccess' })
    } else {
      await createWebDavConfig({
        dav_path: submitDavPath,
        access_path: submitAccessPath,
        password: webdavForm.password,
        permission: webdavForm.permission,
        global_access: webdavForm.global_access,
      })
      ElMessage.success({ __key: 'profile.webdavAddSuccess' })
    }
    webdavDrawerVisible.value = false
    loadWebdavList()
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'common.error' })
    }
  }
}

const handleDeleteWebDav = async (row: WebDavItem) => {
  try {
    await ElMessageBox.confirm(
      t('profile.webdavDeleteConfirm'),
      t('common.confirm'),
      { type: 'warning' }
    )
    await deleteWebDavConfig(row.id)
    ElMessage.success({ __key: 'profile.webdavDeleteSuccess' })
    loadWebdavList()
  } catch {
  }
}

const loadAvatar = async () => {
  if (userStore.user?.id) {
    const blob = await fetchAvatar(userStore.user.id)
    userStore.setAvatar(blob)
  }
}

onMounted(() => {
  if (!userStore.avatarBlob) loadAvatar()
  loadBackupList()
  loadWebdavList()
  initFeatureOrder()
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  if (avatarUrl.value) {
    URL.revokeObjectURL(avatarUrl.value)
  }
})
</script>

<style scoped>
.profile-container {
  padding: 24px;
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
}

.profile-card {
  background: var(--el-bg-color-page);
  border-radius: 16px;
  padding: 24px;
  max-width: 900px;
  margin: 0 auto;
  height: 100%;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  overflow: hidden;
}

.profile-card :deep(.el-tabs) {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.profile-card :deep(.el-tabs__header) {
  flex-shrink: 0;
  margin-bottom: 0;
}

.profile-card :deep(.el-tabs__content) {
  flex: 1;
  overflow: hidden;
  padding: 16px 0 0 0;
}

.profile-card :deep(.el-tab-pane) {
  height: 100%;
  overflow: hidden;
}

.tab-content {
  padding: 0 4px;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

.form-section {
  display: flex;
  flex-direction: column;
  gap: 32px;
}

.form-item {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.basic-info-wrapper {
  width: 100%;
  max-width: 400px;
  margin: 0 auto;
}

.form-input {
  width: 100%;
}

.avatar-section {
  display: flex;
  align-items: center;
  gap: 24px;
  flex-wrap: wrap;
}

.avatar-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.avatar-preview {
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  font-size: 28px;
  font-weight: 600;
}

.password-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.backup-content {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.backup-toolbar {
  display: flex;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.backup-table-wrapper {
  flex: 1;
  overflow: hidden;
}

.backup-table {
  width: 100%;
  height: 100%;
}

.backup-status {
  font-size: 13px;
  font-weight: 500;
}

.backup-status.status-success {
  color: #67c23a;
}

.backup-status.status-failed {
  color: #f56c6c;
}

.backup-status.status-waiting {
  color: #909399;
}

/* 功能排序样式 */
.feature-order-wrapper {
  max-width: 500px;
  margin: 0 auto;
}

.feature-order-tip {
  font-size: 14px;
  color: var(--el-text-color-secondary);
  margin-bottom: 20px;
  text-align: center;
}

.feature-order-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.feature-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-light);
  border-radius: 8px;
  cursor: grab;
  transition: all 0.2s ease;
}

.feature-item:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.feature-item.dragging {
  opacity: 0.5;
  border-color: var(--el-color-primary);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.feature-item:active {
  cursor: grabbing;
}

.drag-handle {
  color: var(--el-text-color-placeholder);
  cursor: grab;
}

.feature-icon {
  color: var(--el-color-primary);
  font-size: 20px;
}

.feature-label {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

/* 移动端适配 */
@media (max-width: 768px) {
  .profile-container {
    padding: 16px;
    height: 100%;
    overflow: hidden;
  }

  .profile-card {
    padding: 16px;
    border-radius: 12px;
    height: 100%;
    overflow: hidden;
  }

  .profile-card :deep(.el-tabs__content) {
    padding: 12px 0 0 0;
  }

  .form-section {
    gap: 24px;
  }

  .avatar-section {
    flex-direction: column;
    align-items: flex-start;
    gap: 16px;
  }

  .avatar-buttons {
    width: 100%;
  }

  .avatar-buttons .el-button {
    flex: 1;
  }

  .backup-toolbar {
    flex-wrap: wrap;
    gap: 8px;
  }

  .backup-toolbar .el-button {
    flex: 1;
    min-width: 120px;
  }

  .backup-content {
    height: 100%;
  }

  .backup-table-wrapper {
    flex: 1;
    overflow: hidden;
  }

  .feature-order-wrapper {
    padding: 0;
  }

  .feature-item {
    padding: 14px 16px;
  }

  .feature-icon {
    font-size: 22px;
  }

  .feature-label {
    font-size: 15px;
  }

  .profile-container .el-link .el-icon {
    margin-right: 0;
  }
}
</style>
