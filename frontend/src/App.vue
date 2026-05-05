<template>
  <el-config-provider :locale="elementLocale">
    <div v-if="loading" class="min-h-screen flex items-center justify-center" :class="isAppPage ? (themeStore.isDark ? 'dark-bg' : 'light-bg') : 'light-bg'">
      <el-icon class="is-loading" :size="48">
        <Loading />
      </el-icon>
    </div>
    <router-view v-else />
  </el-config-provider>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElConfigProvider } from 'element-plus'
import { ElMessage } from '@/utils/message'
import { Loading } from '@element-plus/icons-vue'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import en from 'element-plus/es/locale/lang/en'
import { useI18n } from 'vue-i18n'
import { getSystemInfo } from '@/api'
import { useUserStore } from '@/stores/user'
import { useThemeStore } from '@/stores/theme'

const { locale } = useI18n()
const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const themeStore = useThemeStore()
const loading = ref(true)

const isAppPage = computed(() => {
  const path = route.path
  return path !== '/login' && path !== '/init' && path !== '/'
})

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

onMounted(async () => {
  await router.isReady()
  try {
    const info = await getSystemInfo(true)
    userStore.setSystemInfo(info)
    
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
  } catch {
    loading.value = false
    if (!route.meta.public) {
      ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
      router.push('/login')
    }
  }
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
