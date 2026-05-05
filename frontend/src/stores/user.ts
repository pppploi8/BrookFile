import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { UserInfo, SystemInfoResponse } from '@/api/system'

export const useUserStore = defineStore('user', () => {
  const user = ref<UserInfo | null>(null)
  const loggedIn = ref(false)
  const initialized = ref(false)
  const systemInfoLoaded = ref(false)
  const systemName = ref('BrookFile')
  const avatarBlob = ref<Blob | null>(null)
  const systemLogoUrl = ref<string | null>(null)

  function setSystemInfo(info: SystemInfoResponse) {
    initialized.value = info.initialized
    loggedIn.value = info.logged_in
    user.value = info.user || null
    systemInfoLoaded.value = true
    systemName.value = info.system_name || 'BrookFile'
    document.title = systemName.value
    loadSystemLogo()
  }

  async function loadSystemLogo() {
    try {
      const resp = await fetch('/api/system/logo', { credentials: 'include' })
      const contentType = resp.headers.get('Content-Type') || ''
      if (resp.ok && contentType.startsWith('image/')) {
        const blob = await resp.blob()
        systemLogoUrl.value = URL.createObjectURL(blob)
      } else {
        systemLogoUrl.value = null
      }
    } catch {
      systemLogoUrl.value = null
    }
  }

  function setLoggedIn(status: boolean, userInfo?: UserInfo) {
    loggedIn.value = status
    user.value = userInfo || null
  }

  function logout() {
    loggedIn.value = false
    user.value = null
    avatarBlob.value = null
  }

  function setInitialized(status: boolean) {
    initialized.value = status
  }

  function setAvatar(blob: Blob | null) {
    avatarBlob.value = blob
  }

  function setFeatureOrder(order: string) {
    if (user.value) {
      user.value = { ...user.value, feature_order: order }
    }
  }

  function setHasShares(enabled: boolean) {
    if (user.value) {
      user.value = { ...user.value, has_shares: enabled }
    }
  }

  return {
    user,
    loggedIn,
    initialized,
    systemInfoLoaded,
    systemName,
    avatarBlob,
    systemLogoUrl,
    setSystemInfo,
    setLoggedIn,
    logout,
    setInitialized,
    setAvatar,
    setFeatureOrder,
    setHasShares,
    loadSystemLogo,
  }
})
