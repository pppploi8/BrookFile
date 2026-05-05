<template>
  <div class="login-page">
    <div class="login-card">
      <div class="login-header">
        <div class="logo">
          <img v-if="userStore.systemLogoUrl" :src="userStore.systemLogoUrl" class="logo-image" alt="Logo" />
          <svg v-else width="40" height="40" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 4L4 8V20L12 24L20 20V8L12 4Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M12 12V24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <h1>{{ userStore.systemName }}</h1>
      </div>
      
      <div class="login-body">
        <div class="form-title">{{ t('init.title') }}</div>
        
        <el-form ref="formRef" :model="form" :rules="rules" label-position="top" @submit.prevent="handleInit">
          <el-form-item prop="systemName" :label="t('init.systemName')">
            <el-input 
              v-model="form.systemName" 
              :placeholder="t('init.systemNamePlaceholder')"
            />
          </el-form-item>
          
          <el-form-item prop="username" :label="t('init.username')">
            <el-input 
              v-model="form.username" 
              :placeholder="t('init.usernamePlaceholder')"
              autocomplete="username"
            />
          </el-form-item>
          
          <el-form-item prop="password" :label="t('init.password')">
            <el-input 
              v-model="form.password" 
              type="password"
              :placeholder="t('init.passwordPlaceholder')"
              autocomplete="new-password"
            />
          </el-form-item>
          
          <el-form-item prop="confirmPassword" :label="t('init.confirmPassword')">
            <el-input 
              v-model="form.confirmPassword" 
              type="password"
              :placeholder="t('init.confirmPasswordPlaceholder')"
              autocomplete="new-password"
            />
          </el-form-item>
          
          <el-form-item prop="rootPath" :label="t('init.rootPath')">
            <div class="path-selector">
              <el-input 
                v-model="form.rootPath" 
                :placeholder="t('init.rootPathPlaceholder')"
                class="path-input"
              />
              <el-button type="primary" @click="openFolderDialog('rootPath')">
                {{ t('init.browse') }}
              </el-button>
            </div>
          </el-form-item>
          
          <el-form-item prop="recycleBinPath" :label="t('init.recycleBinPath')">
            <div class="path-selector">
              <el-input 
                v-model="form.recycleBinPath" 
                :placeholder="t('init.recycleBinPathPlaceholder')"
                class="path-input"
                clearable
              />
              <el-button type="primary" @click="openFolderDialog('recycleBinPath')">
                {{ t('init.browse') }}
              </el-button>
            </div>
          </el-form-item>
          
          <el-form-item>
            <el-button type="primary" class="login-btn" :loading="loading" native-type="submit">
              {{ loading ? t('init.loading') : t('init.submit') }}
            </el-button>
          </el-form-item>
        </el-form>
      </div>
    </div>
    
    <el-dialog
      v-model="folderDialogVisible"
      :title="t('init.selectFolder')"
      width="min(500px, calc(100vw - 32px))"
      :close-on-click-modal="false"
    >
      <div class="folder-browser" v-loading="browseLoading">
        <div class="folder-nav">
          <el-button 
            :disabled="!browseData.has_parent" 
            @click="goToParent"
            size="small"
          >
            {{ t('init.parentFolder') }}
          </el-button>
          <div class="current-path">{{ currentBrowsePath || t('init.rootDirectory') }}</div>
        </div>
        
        <div class="folder-list">
          <div 
            v-for="folder in browseData.folders" 
            :key="folder.path"
            class="folder-item"
            @click="enterFolder(folder)"
          >
            <el-icon size="20"><Folder /></el-icon>
            <span>{{ folder.name }}</span>
          </div>
          <div v-if="browseData.folders.length === 0 && !browseLoading" class="empty-folder">
            {{ t('init.noFolders') }}
          </div>
        </div>
      </div>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="folderDialogVisible = false">{{ t('common.cancel') }}</el-button>
          <el-button type="primary" @click="confirmFolderSelection">
            {{ t('common.confirm') }}
          </el-button>
        </span>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { type FormInstance, type FormRules } from 'element-plus'
import { Folder } from '@element-plus/icons-vue'
import { initSystem, browseFolders, getSystemInfo } from '@/api'
import { useUserStore } from '@/stores/user'
import type { BrowseResponse } from '@/api/system'

const { t } = useI18n()
const router = useRouter()
const userStore = useUserStore()

const formRef = ref<FormInstance>()
const loading = ref(false)
const folderDialogVisible = ref(false)
const browseLoading = ref(false)
const currentBrowsePath = ref('')
const browseTarget = ref<'rootPath' | 'recycleBinPath'>('rootPath')

const browseData = reactive<BrowseResponse>({
  folders: [],
  has_parent: false,
  parent_path: null
})

const form = reactive({
  systemName: '',
  username: '',
  password: '',
  confirmPassword: '',
  rootPath: '',
  recycleBinPath: '',
})

const validateConfirmPassword = (_rule: unknown, value: string, callback: (error?: Error) => void) => {
  if (value !== form.password) {
    callback(new Error(t('init.passwordMismatch')))
  } else {
    callback()
  }
}

const rules = reactive<FormRules>({
  systemName: [
    { required: true, message: t('init.pleaseEnterSystemName'), trigger: 'blur' },
  ],
  username: [
    { required: true, message: t('init.pleaseEnterUsername'), trigger: 'blur' },
  ],
  password: [
    { required: true, message: t('init.pleaseEnterPassword'), trigger: 'blur' },
  ],
  confirmPassword: [
    { required: true, message: t('init.pleaseEnterConfirmPassword'), trigger: 'blur' },
    { validator: validateConfirmPassword, trigger: 'blur' },
  ],
  rootPath: [
    { required: true, message: t('init.pleaseSelectRootPath'), trigger: 'change' },
  ],
})

onMounted(() => {
  form.systemName = userStore.systemName
})

const openFolderDialog = async (target: 'rootPath' | 'recycleBinPath') => {
  browseTarget.value = target
  folderDialogVisible.value = true
  await loadFolders(target === 'rootPath' ? form.rootPath : form.recycleBinPath || currentBrowsePath.value)
}

const loadFolders = async (path: string) => {
  browseLoading.value = true
  try {
    const data = await browseFolders(path || undefined)
    browseData.folders = data.folders.sort((a, b) => a.name.localeCompare(b.name))
    browseData.has_parent = data.has_parent
    browseData.parent_path = data.parent_path
    currentBrowsePath.value = path
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
    }
  } finally {
    browseLoading.value = false
  }
}

const goToParent = () => {
  if (browseData.has_parent && browseData.parent_path !== null) {
    loadFolders(browseData.parent_path)
  }
}

const enterFolder = (folder: { path: string }) => {
  loadFolders(folder.path)
}

const confirmFolderSelection = () => {
  if (currentBrowsePath.value) {
    if (browseTarget.value === 'rootPath') {
      form.rootPath = currentBrowsePath.value
    } else {
      form.recycleBinPath = currentBrowsePath.value
    }
  }
  folderDialogVisible.value = false
}

const handleInit = async () => {
  if (!formRef.value) return
  
  loading.value = true
  try {
    const valid = await formRef.value.validate()
    if (!valid) return
    
    await initSystem({
      username: form.username,
      password: form.password,
      system_name: form.systemName,
      root_path: form.rootPath,
      recycle_bin_path: form.recycleBinPath || undefined,
    })
    const info = await getSystemInfo()
    userStore.setSystemInfo(info)
    router.push('/login')
  } catch (error: any) {
    if (error.message === 'SYSTEM_ALREADY_INITIALIZED') {
      router.push('/login')
    } else if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
    }
  } finally {
    loading.value = false
  }
}
</script>

<style scoped>
.login-page {
  min-height: 100vh;
  min-height: 100dvh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #e0f2fe 0%, #bae6fd 50%, #7dd3fc 100%);
  padding: 16px;
}

.login-card {
  width: 100%;
  max-width: 420px;
  background: #ffffff;
  border-radius: 16px;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.1);
  overflow: hidden;
}

.login-header {
  padding: 32px 24px 24px;
  text-align: center;
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  color: #ffffff;
}

.logo {
  width: 56px;
  height: 56px;
  margin: 0 auto 12px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #ffffff;
}

.logo-image {
  width: 56px;
  height: 56px;
  object-fit: contain;
  border-radius: 12px;
}

.login-header h1 {
  font-size: 22px;
  font-weight: 600;
}

.login-body {
  padding: 24px;
}

.form-title {
  font-size: 16px;
  color: #374151;
  margin-bottom: 20px;
  font-weight: 500;
}

.path-selector {
  display: flex;
  gap: 8px;
  width: 100%;
}

.path-input {
  flex: 1;
}

.login-btn {
  width: 100%;
}

.folder-browser {
  min-height: 250px;
}

.folder-nav {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid #e5e7eb;
}

.current-path {
  flex: 1;
  font-size: 13px;
  color: #6b7280;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.folder-list {
  max-height: 200px;
  overflow-y: auto;
}

.folder-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.folder-item:hover {
  background-color: #f0f9ff;
}

.folder-item span {
  font-size: 14px;
  color: #374151;
}

.empty-folder {
  text-align: center;
  color: #9ca3af;
  padding: 40px 0;
  font-size: 14px;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

@media (min-width: 640px) {
  .login-page {
    padding: 0;
  }

  .login-header {
    padding: 32px 32px 24px;
  }

  .login-header h1 {
    font-size: 24px;
  }

  .login-body {
    padding: 32px;
  }

  .form-title {
    margin-bottom: 24px;
  }

  .folder-browser {
    min-height: 300px;
  }

  .folder-list {
    max-height: 250px;
  }
}
</style>
