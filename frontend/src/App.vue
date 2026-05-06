<template>
  <el-config-provider :locale="elementLocale">
    <div v-if="loading" class="min-h-screen flex flex-col items-center justify-center light-bg">
      <el-icon class="is-loading" :size="48">
        <Loading />
      </el-icon>
      <p v-if="loadFailed" class="mt-4 text-gray-400">{{ t('app.loadFailed') }}</p>
    </div>
    <router-view v-else />
  </el-config-provider>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElConfigProvider } from 'element-plus'
import { Loading } from '@element-plus/icons-vue'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import en from 'element-plus/es/locale/lang/en'
import { useI18n } from 'vue-i18n'
import { getSystemInfo } from '@/api'
import { useUserStore } from '@/stores/user'
import { useThemeStore } from '@/stores/theme'

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const themeStore = useThemeStore()
const loading = ref(true)
const loadFailed = ref(false)
let retryTimer: ReturnType<typeof setInterval> | null = null

const elementLocale = computed(() => {
  return locale.value === 'zh' ? zhCn : en
})

watch(() => route.path, (newPath) => {
  const isMainApp = newPath !== '/login' && newPath !== '/init' && newPath !== '/'
  if (isMainApp) {
    themeStore.initTheme()
  } else {
    themeStore.resetToLight()
  }
}, { immediate: true })

function clearRetryTimer() {
  if (retryTimer !== null) {
    clearInterval(retryTimer)
    retryTimer = null
  }
}

async function loadSystemInfo() {
  const info = await getSystemInfo(true)
  userStore.setSystemInfo(info)
  clearRetryTimer()
  loadFailed.value = false

  if (!info.initialized) {
    loading.value = false
    router.push('/init')
  } else if (!info.logged_in) {
    loading.value = false
    if (route.meta.public) {
      return
    }
    router.push('/login')
  } else {
    if (route.path === '/login' || route.path === '/init') {
      await router.push('/')
    }
    themeStore.initTheme()
    loading.value = false
  }
}

onMounted(async () => {
  await router.isReady()
  try {
    await loadSystemInfo()
  } catch {
    loadFailed.value = true
    retryTimer = setInterval(async () => {
      try {
        await loadSystemInfo()
      } catch {
        // continue retrying
      }
    }, 5000)
  }
})

onUnmounted(() => {
  clearRetryTimer()
})
</script>

<style>
.light-bg {
  background: #f5f7fa;
}

.dark-bg {
  background: #0f0f12;
}
</style>
