<template>
  <div class="share-page">
    <div class="share-card" v-if="errorType">
      <div class="error-icon">
        <el-icon :size="64"><CircleCloseFilled /></el-icon>
      </div>
      <h2>{{ errorMessage }}</h2>
    </div>

    <div class="share-card" v-else-if="shareInfo">
      <div class="share-header">
        <div class="logo">
          <img v-if="userStore.systemLogoUrl" :src="userStore.systemLogoUrl" class="logo-image" alt="Logo" />
          <svg v-else width="40" height="40" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M3 7V17C3 18.1046 3.89543 19 5 19H19C20.1046 19 21 18.1046 21 17V9C21 7.89543 20.1046 7 19 7H13L11 5H5C3.89543 5 3 5.89543 3 7Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <h1>{{ userStore.systemName }}</h1>
      </div>

      <div class="file-info">
        <el-icon class="file-type-icon" :class="shareInfo.is_directory ? 'is-folder' : ''">
          <Folder v-if="shareInfo.is_directory" />
          <Document v-else />
        </el-icon>
        <div class="file-detail">
          <div class="file-name">{{ shareInfo.file_name }}</div>
          <div class="file-meta" v-if="shareInfo.file_size != null">
            {{ formatFileSize(shareInfo.file_size) }}
          </div>
          <div class="file-meta" v-if="shareInfo.created_at">
            {{ t('share.createdTime') }}: {{ formatUtcDatetimeString(shareInfo.created_at) }}
          </div>
        </div>
      </div>

      <div class="password-section" v-if="shareInfo.need_password">
        <el-input
          v-model="password"
          type="password"
          :placeholder="t('share.passwordPlaceholder')"
          show-password
          @keyup.enter="handleDownload"
        />
      </div>

      <el-button
        type="primary"
        class="download-btn"
        :loading="downloading"
        @click="handleDownload"
      >
        {{ shareInfo.is_directory ? t('share.downloadFolderButton') : t('share.downloadButton') }}
      </el-button>
    </div>

    <div class="share-card loading-card" v-else>
      <el-icon class="is-loading" :size="40"><Loading /></el-icon>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Folder, Document, Loading, CircleCloseFilled } from '@element-plus/icons-vue'
import { ElMessage } from '@/utils/message'
import CryptoJS from 'crypto-js'
import { formatUtcDatetimeString } from '@/utils/date'
import { formatFileSize } from '@/utils/format'
import { getShareInfo, getShareDownloadToken, type ShareInfoResponse } from '@/api/system'
import { useUserStore } from '@/stores/user'

const { t } = useI18n()
const route = useRoute()
const userStore = useUserStore()

const code = computed(() => route.params.code as string)
const shareInfo = ref<ShareInfoResponse | null>(null)
const errorType = ref('')
const password = ref('')
const downloading = ref(false)

const errorMessage = computed(() => {
  const map: Record<string, string> = {
    SHARE_NOT_FOUND: t('share.shareNotExistPage'),
    SHARE_EXPIRED: t('share.shareExpiredMsg'),
    SHARE_FILE_MISSING: t('share.fileDeleted'),
    SHARE_OVER_LIMIT: t('share.downloadLimitReached'),
  }
  return map[errorType.value] || t('share.shareNotExistPage')
})

const loadShareInfo = async () => {
  try {
    const res = await getShareInfo({ share_code: code.value })
    if (res.success) {
      shareInfo.value = res
    } else {
      errorType.value = res.fail_code || 'SHARE_NOT_FOUND'
    }
  } catch {
    errorType.value = 'SHARE_NOT_FOUND'
  }
}

const handleDownload = async () => {
  downloading.value = true
  try {
    const reqData: { share_code: string; password_hash?: string } = { share_code: code.value }

    if (shareInfo.value?.need_password) {
      if (!password.value) {
        ElMessage.warning({ __key: 'share.passwordRequired' })
        downloading.value = false
        return
      }
      const salt = shareInfo.value.password_salt || ''
      const hash = CryptoJS.HmacSHA256(password.value, salt).toString()
      reqData.password_hash = hash
    }

    const tokenRes = await getShareDownloadToken(reqData)
    if (!tokenRes.success) {
      if (tokenRes.fail_code === 'SHARE_PASSWORD_WRONG') {
        ElMessage.error({ __key: 'share.passwordWrong' })
      } else if (tokenRes.fail_code === 'SHARE_PASSWORD_REQUIRED') {
        ElMessage.warning({ __key: 'share.passwordRequired' })
      } else {
        errorType.value = tokenRes.fail_code || 'SHARE_NOT_FOUND'
      }
      downloading.value = false
      return
    }

    const url = `/api/share/file/${code.value}?token=${encodeURIComponent(tokenRes.download_token!)}`
    const link = document.createElement('a')
    link.href = url
    link.download = ''
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
  } catch {
    ElMessage.error({ __key: 'errors.INTERNAL_ERROR' })
    downloading.value = false
    return
  }
  downloading.value = false
}

onMounted(() => {
  loadShareInfo()
})

watch(code, () => {
  shareInfo.value = null
  errorType.value = ''
  password.value = ''
  downloading.value = false
  loadShareInfo()
})
</script>

<style scoped>
.share-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--el-bg-color-page);
  padding: 20px;
  box-sizing: border-box;
}

.share-card {
  background: var(--el-bg-color);
  border-radius: 12px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.1);
  padding: 40px;
  width: 100%;
  max-width: 420px;
  text-align: center;
}

.loading-card {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 200px;
}

.share-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-bottom: 32px;
}

.logo {
  color: var(--el-color-primary);
  margin-bottom: 12px;
}

.logo-image {
  width: 40px;
  height: 40px;
  object-fit: contain;
  border-radius: 8px;
}

.share-header h1 {
  margin: 0;
  font-size: 20px;
  color: var(--el-text-color-primary);
}

.error-icon {
  color: var(--el-color-danger);
  margin-bottom: 16px;
}

.error-icon + h2 {
  margin: 0;
  font-size: 16px;
  color: var(--el-text-color-regular);
  font-weight: normal;
}

.file-info {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 20px;
  background: var(--el-fill-color-light);
  border-radius: 8px;
  margin-bottom: 24px;
  text-align: left;
}

.file-type-icon {
  font-size: 40px;
  color: var(--el-color-primary);
  flex-shrink: 0;
}

.file-type-icon.is-folder {
  color: #e6a23c;
}

.file-detail {
  flex: 1;
  min-width: 0;
}

.file-name {
  font-size: 16px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  word-break: break-all;
}

.file-meta {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
}

.password-section {
  margin-bottom: 20px;
}

.download-btn {
  width: 100%;
}
</style>
