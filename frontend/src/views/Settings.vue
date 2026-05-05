<template>
  <div class="profile-container">
    <div class="profile-card">
      <el-tabs model-value="settings" class="profile-tabs" style="height: 100%">
        <el-tab-pane :label="t('settings.title')" name="settings">
          <div class="tab-content">
            <div class="basic-info-wrapper">

              <div class="form-section">
                <div class="form-item">
                  <label class="form-label">{{ t('settings.systemLogo') }}</label>
                  <div class="avatar-section">
                    <div class="logo-preview logo-preview--image" v-if="logoUrl">
                      <img :src="logoUrl" alt="Logo" />
                    </div>
                    <div class="logo-preview logo-preview--default" v-else>
                      <svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" width="24" height="24" color="white">
                        <path d="M3 7V17C3 18.1046 3.89543 19 5 19H19C20.1046 19 21 18.1046 21 17V9C21 7.89543 20.1046 7 19 7H13L11 5H5C3.89543 5 3 5.89543 3 7Z" fill="currentColor"/>
                      </svg>
                    </div>
                    <div class="avatar-buttons">
                      <input
                        ref="logoInputRef"
                        type="file"
                        accept="image/jpeg,image/png,image/gif,image/webp,image/svg+xml"
                        style="display: none"
                        @change="handleLogoSelect"
                      />
                      <el-button type="primary" plain @click="triggerLogoUpload">
                        {{ logoUrl ? t('settings.changeLogo') : t('settings.uploadLogo') }}
                      </el-button>
                      <el-button type="danger" plain v-if="logoUrl" @click="handleDeleteLogo">
                        {{ t('settings.deleteLogo') }}
                      </el-button>
                    </div>
                  </div>
                </div>

                <div class="form-item">
                  <label class="form-label">{{ t('settings.systemName') }}</label>
                  <el-input v-model="form.system_name" class="form-input" />
                </div>

                <div class="form-item">
                  <label class="form-label">{{ t('settings.sessionTimeout') }} <span class="restart-hint">{{ t('settings.restartRequired') }}</span></label>
                  <el-input-number v-model="form.session_timeout" :min="60" :step="60" />
                </div>

                <div class="form-item">
                  <label class="form-label">{{ t('settings.fulltextSearch') }} <span class="restart-hint">{{ t('settings.restartRequired') }}</span></label>
                  <div class="switch-row">
                    <el-switch v-model="form.notebook_fulltext_search" />
                    <span class="switch-tip">{{ t('settings.fulltextSearchTip') }}</span>
                  </div>
                </div>

                <div class="form-item">
                  <label class="form-label">{{ t('settings.rebuildIndex') }}</label>
                  <el-button @click="handleRebuildIndex" :loading="rebuilding">
                    {{ t('settings.rebuildIndex') }}
                  </el-button>
                </div>
              </div>

              <div style="margin-top: 32px;">
                <el-button type="primary" @click="handleSave" :loading="saving">
                  {{ t('settings.save') }}
                </el-button>
              </div>

            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useUserStore } from '@/stores/user'
import {
  getSystemSettings,
  updateSystemSettings,
  rebuildNotebookIndex,
  uploadSystemLogo,
  deleteSystemLogo
} from '@/api/system'

const { t } = useI18n()
const userStore = useUserStore()

const form = ref({
  system_name: '',
  session_timeout: 1800,
  notebook_fulltext_search: true,
})
const logoUrl = ref<string | null>(null)
const saving = ref(false)
const rebuilding = ref(false)
const logoInputRef = ref<HTMLInputElement>()

onMounted(async () => {
  logoUrl.value = userStore.systemLogoUrl
  try {
    const res = await getSystemSettings()
    if (res.success) {
      form.value.system_name = res.system_name
      form.value.session_timeout = res.session_timeout
      form.value.notebook_fulltext_search = res.notebook_fulltext_search
    }
  } catch {}
})

function triggerLogoUpload() {
  logoInputRef.value?.click()
}

async function handleLogoSelect(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  input.value = ''
  try {
    const res = await uploadSystemLogo(file)
    if (res.success) {
      ElMessage.success(t('settings.logoUploadSuccess'))
      await userStore.loadSystemLogo()
      logoUrl.value = userStore.systemLogoUrl
    }
  } catch {}
}

async function handleDeleteLogo() {
  try {
    const res = await deleteSystemLogo()
    if (res.success) {
      ElMessage.success(t('settings.logoDeleteSuccess'))
      userStore.systemLogoUrl = null
      logoUrl.value = null
    }
  } catch {}
}

async function handleSave() {
  saving.value = true
  try {
    await updateSystemSettings(form.value)
    userStore.systemName = form.value.system_name
    document.title = form.value.system_name
    ElMessage.success(t('settings.saveSuccess'))
  } catch {} finally {
    saving.value = false
  }
}

async function handleRebuildIndex() {
  try {
    await ElMessageBox.confirm(t('settings.rebuildIndexConfirm'), { type: 'warning' })
  } catch { return }
  rebuilding.value = true
  try {
    await rebuildNotebookIndex()
    ElMessage.success(t('settings.rebuildIndexStarted'))
  } catch {} finally {
    rebuilding.value = false
  }
}
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

.basic-info-wrapper {
  width: 100%;
  max-width: 400px;
  margin: 0 auto;
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

.form-input {
  width: 100%;
}

.restart-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: normal;
  margin-left: 8px;
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
}

.logo-preview {
  width: 64px;
  height: 64px;
  border-radius: 12px;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.logo-preview--default {
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  box-shadow: 0 4px 12px rgba(14, 165, 233, 0.3);
}

.logo-preview--image img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.logo-preview--default svg {
  color: white;
}

.switch-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.switch-tip {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>
