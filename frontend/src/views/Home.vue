<template>
  <div class="home-container" :class="{ dark: themeStore.isDark, 'mobile-layout': isMobileLayout }">
    <!-- PC端侧边栏 -->
    <aside class="sidebar" v-if="!isMobileLayout">
      <div class="sidebar-header">
        <div class="logo">
          <div class="logo-icon" v-if="!userStore.systemLogoUrl">
            <svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M3 7V17C3 18.1046 3.89543 19 5 19H19C20.1046 19 21 18.1046 21 17V9C21 7.89543 20.1046 7 19 7H13L11 5H5C3.89543 5 3 5.89543 3 7Z" fill="currentColor"/>
            </svg>
          </div>
          <img v-else :src="userStore.systemLogoUrl" class="logo-image" alt="Logo" />
          <span class="logo-text">{{ userStore.systemName }}</span>
        </div>
      </div>

      <nav class="sidebar-nav">
        <ul class="nav-list">
          <template v-for="(item, index) in menuItems" :key="item.path">
            <li v-if="item.fixed && !menuItems.slice(0, index).some(m => m.fixed)" class="nav-divider-wrapper">
              <div class="nav-divider"></div>
            </li>
            <li>
              <router-link
                :to="item.path"
                class="nav-item"
                :class="{ active: isActive(item.path) }"
              >
                <div class="nav-icon">
                  <el-icon :size="22">
                    <component :is="item.icon" />
                  </el-icon>
                </div>
                <span class="nav-label">{{ item.label }}</span>
                <div class="nav-indicator" v-if="isActive(item.path)"></div>
              </router-link>
            </li>
          </template>
        </ul>
      </nav>
    </aside>

    <div class="main-content">
      <!-- PC端头部 -->
      <header class="header" v-if="!isMobileLayout">
        <div class="header-left">
          <h2 class="page-title">
            {{ currentCategoryLabel }}
          </h2>
        </div>

        <div class="header-right">
          <div class="header-actions">
            <el-dropdown trigger="click" @command="handleLangCommand">
              <div class="action-btn lang-btn">
                <svg class="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/>
                  <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
                </svg>
              </div>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item :command="'zh'" :class="{ active: locale === 'zh' }">中文</el-dropdown-item>
                  <el-dropdown-item :command="'en'" :class="{ active: locale === 'en' }">English</el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>

            <div class="action-btn theme-btn" @click="themeStore.toggleTheme">
              <el-icon v-if="themeStore.isDark"><Sunny /></el-icon>
              <el-icon v-else><Moon /></el-icon>
            </div>
          </div>

          <el-dropdown trigger="click" @command="handleCommand">
            <div class="user-menu">
              <el-avatar :size="36" class="header-avatar" :src="avatarUrl">
                <template v-if="!avatarUrl">
                  {{ userStore.user?.username?.charAt(0).toUpperCase() }}
                </template>
              </el-avatar>
              <span class="header-username">{{ userStore.user?.username }}</span>
              <el-icon class="dropdown-arrow"><ArrowDown /></el-icon>
            </div>
            <template #dropdown>
              <el-dropdown-menu class="custom-dropdown">
                <el-dropdown-item command="profile">
                  <el-icon><UserFilled /></el-icon>
                  <span>{{ t('home.profileCenter') }}</span>
                </el-dropdown-item>
                <el-dropdown-item command="settings" v-if="userStore.user?.is_admin">
                  <el-icon><Setting /></el-icon>
                  <span>{{ t('home.settings') }}</span>
                </el-dropdown-item>
                <el-dropdown-item command="accountManagement" v-if="userStore.user?.is_admin">
                  <el-icon><User /></el-icon>
                  <span>{{ t('home.accountManagement') }}</span>
                </el-dropdown-item>
                <el-dropdown-item command="logout" divided>
                  <el-icon><SwitchButton /></el-icon>
                  <span>{{ t('home.logout') }}</span>
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </header>

      <!-- 移动端头部 -->
      <header class="mobile-header" v-if="isMobileLayout">
        <div class="mobile-header-content">
          <el-avatar :size="32" class="mobile-avatar" :src="avatarUrl" @click="showUserMenu = true">
            <template v-if="!avatarUrl">
              {{ userStore.user?.username?.charAt(0).toUpperCase() }}
            </template>
          </el-avatar>

          <h2 class="mobile-title">{{ currentCategoryLabel }}</h2>

          <div class="mobile-actions">
            <div class="mobile-action-btn" @click="themeStore.toggleTheme">
              <el-icon :size="20"><Sunny v-if="themeStore.isDark" /><Moon v-else /></el-icon>
            </div>
          </div>
        </div>
      </header>

      <main class="content-area">
        <div v-if="routeLoading" class="route-loading">
          <div class="route-loading-spinner"></div>
        </div>
        <router-view v-else />
      </main>

      <!-- 移动端底部导航 -->
      <nav class="mobile-nav" v-if="isMobileLayout">
        <router-link
          v-for="item in mobileNavItems"
          :key="item.path"
          :to="item.path"
          class="mobile-nav-item"
          :class="{ active: isActive(item.path) }"
        >
          <el-icon :size="22">
            <component :is="item.icon" />
          </el-icon>
          <span class="mobile-nav-label">{{ item.label }}</span>
        </router-link>

        <div class="mobile-nav-item more-btn" @click="showMoreMenu = true">
          <el-icon :size="22"><More /></el-icon>
          <span class="mobile-nav-label">{{ t('home.more') }}</span>
        </div>
      </nav>

      <!-- 移动端更多菜单弹出层 -->
      <transition name="more-menu-fade">
        <div class="more-menu-overlay" v-if="showMoreMenu" @click="showMoreMenu = false">
          <div class="more-menu-popup" @click.stop>
            <div class="more-menu-grid">
              <div
                v-for="item in moreMenuItems"
                :key="item.path"
                class="more-menu-item"
                :class="{ active: isActive(item.path) }"
                @click="handleMoreMenuClick(item.path)"
              >
                <el-icon :size="24">
                  <component :is="item.icon" />
                </el-icon>
                <span>{{ item.label }}</span>
              </div>
            </div>
          </div>
        </div>
      </transition>
    </div>

    <!-- 移动端用户菜单抽屉 -->
    <el-drawer
      v-model="showUserMenu"
      direction="ltr"
      size="280px"
      :with-header="false"
      :body-style="{ padding: 0 }"
      v-if="isMobileLayout"
    >
      <div class="mobile-user-drawer">
        <div class="drawer-header">
          <el-avatar :size="56" class="drawer-avatar" :src="avatarUrl">
            <template v-if="!avatarUrl">
              {{ userStore.user?.username?.charAt(0).toUpperCase() }}
            </template>
          </el-avatar>
          <div class="drawer-user-info">
            <div class="drawer-username">{{ userStore.user?.username }}</div>
            <div class="drawer-system">{{ userStore.systemName }}</div>
          </div>
        </div>

        <div class="drawer-menu">
          <div class="drawer-menu-item" @click="handleDrawerCommand('profile')">
            <el-icon><UserFilled /></el-icon>
            <span>{{ t('home.profileCenter') }}</span>
          </div>
          <div class="drawer-menu-item" v-if="userStore.user?.is_admin" @click="handleDrawerCommand('settings')">
            <el-icon><Setting /></el-icon>
            <span>{{ t('home.settings') }}</span>
          </div>
          <div class="drawer-menu-item" v-if="userStore.user?.is_admin" @click="handleDrawerCommand('accountManagement')">
            <el-icon><User /></el-icon>
            <span>{{ t('home.accountManagement') }}</span>
          </div>
          <div class="drawer-divider"></div>
          <div class="drawer-menu-item lang-item">
            <el-icon><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="18" height="18">
              <circle cx="12" cy="12" r="10"/>
              <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
            </svg></el-icon>
            <span>{{ t('home.language') }}</span>
            <div class="lang-options">
              <span :class="{ active: locale === 'zh' }" @click="handleLangCommand('zh')">中文</span>
              <span :class="{ active: locale === 'en' }" @click="handleLangCommand('en')">EN</span>
            </div>
          </div>
          <div class="drawer-divider"></div>
          <div class="drawer-menu-item logout-item" @click="handleDrawerCommand('logout')">
            <el-icon><SwitchButton /></el-icon>
            <span>{{ t('home.logout') }}</span>
          </div>
        </div>
      </div>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessageBox } from 'element-plus'
import {
  ArrowDown,
  Setting,
  SwitchButton,
  Document,
  Sunny,
  Moon,
  EditPen,
  Key,
  User,
  UserFilled,
  More,
  Share,
  Delete
} from '@element-plus/icons-vue'
import { logout, fetchAvatar } from '@/api'
import { useUserStore } from '@/stores/user'
import { useThemeStore } from '@/stores/theme'

const { t, locale } = useI18n()
const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const themeStore = useThemeStore()

const showUserMenu = ref(false)
const showMoreMenu = ref(false)
const isMobileLayout = ref(false)
const routeLoading = ref(false)
const pendingPath = ref<string | null>(null)

let routeLoadingTimer: ReturnType<typeof setTimeout> | null = null

const beforeGuard = router.beforeEach((to, from) => {
  if (to.path !== from.path && to.path.startsWith('/')) {
    pendingPath.value = to.path
    routeLoading.value = true
    if (routeLoadingTimer) clearTimeout(routeLoadingTimer)
    routeLoadingTimer = setTimeout(() => {
      routeLoading.value = false
    }, 10000)
  }
})
router.afterEach(() => {
  pendingPath.value = null
  if (routeLoadingTimer) { clearTimeout(routeLoadingTimer); routeLoadingTimer = null }
  nextTick(() => { routeLoading.value = false })
})

// 根据屏幕长宽比判断是否使用移动端布局
// 当高度大于宽度的1.2倍时使用移动端布局
const checkLayout = () => {
  const width = window.innerWidth
  const height = window.innerHeight
  const aspectRatio = height / width
  isMobileLayout.value = aspectRatio > 1.2 || width < 768
}

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

onUnmounted(() => {
  if (avatarUrl.value) {
    URL.revokeObjectURL(avatarUrl.value)
  }
  if (routeLoadingTimer) { clearTimeout(routeLoadingTimer); routeLoadingTimer = null }
  beforeGuard()
  window.removeEventListener('resize', checkLayout)
})

const menuItems = computed(() => {
  const allItems: { key: string; path: string; label: string; icon: typeof Document; fixed: boolean }[] = [
    { key: 'file', path: '/files', label: t('home.categories.files'), icon: Document, fixed: false },
    { key: 'note', path: '/notes', label: t('home.categories.notes'), icon: EditPen, fixed: false },
    { key: 'password', path: '/passwords', label: t('home.categories.passwords'), icon: Key, fixed: false },
  ]

  const featureOrder = userStore.user?.feature_order
  if (featureOrder) {
    const order = featureOrder.split(',')
    const validKeys = allItems.map(item => item.key)
    const orderedItems = order
      .filter(key => validKeys.includes(key))
      .map(key => allItems.find(item => item.key === key))
      .filter((item): item is typeof allItems[0] => item !== undefined)
    allItems.forEach(item => {
      if (!order.includes(item.key)) {
        orderedItems.push(item)
      }
    })
    allItems.length = 0
    allItems.push(...orderedItems)
  }

  const showShare = !!userStore.user?.has_shares
  const showRecycleBin = !!userStore.user?.recycle_bin_enabled

  if (showShare) {
    allItems.push({ key: 'share', path: '/shares', label: t('share.title'), icon: Share, fixed: true })
  }

  if (showRecycleBin) {
    allItems.push({ key: 'recycle_bin', path: '/recycle-bin', label: t('home.categories.recycleBin'), icon: Delete, fixed: true })
  }

  return allItems
})

// 移动端底部导航：前3个 + 更多
const mobileNavItems = computed(() => menuItems.value.slice(0, 3))
const moreMenuItems = computed(() => menuItems.value.slice(3))

const currentCategoryLabel = computed(() => {
  const currentPath = pendingPath.value || route.path
  if (currentPath === '/account-management') {
    return t('home.accountManagement')
  }
  if (currentPath === '/profile') {
    return t('home.profileCenter')
  }
  if (currentPath === '/settings') {
    return t('home.settings')
  }
  if (currentPath === '/recycle-bin') {
    return t('home.categories.recycleBin')
  }
  const item = menuItems.value.find(m => currentPath.startsWith(m.path))
  return item?.label || ''
})

const isActive = (path: string) => {
  const currentPath = pendingPath.value || route.path
  return currentPath.startsWith(path)
}

const handleLangCommand = (command: string) => {
  locale.value = command
  localStorage.setItem('locale', command)
}

const handleCommand = async (command: string) => {
  if (command === 'logout') {
    try {
      await ElMessageBox.confirm(
        t('home.logout') + '?',
        t('common.confirm'),
        {
          type: 'warning',
        }
      )
      await logout()
      userStore.logout()
      router.push('/login')
    } catch {
    }
  } else if (command === 'settings') {
    router.push('/settings')
  } else if (command === 'accountManagement') {
    router.push('/account-management')
  } else if (command === 'profile') {
    router.push('/profile')
  }
}

const handleDrawerCommand = async (command: string) => {
  showUserMenu.value = false
  await handleCommand(command)
}

const handleMoreMenuClick = (path: string) => {
  showMoreMenu.value = false
  router.push(path)
}

const loadAvatar = async () => {
  if (userStore.user?.id) {
    const blob = await fetchAvatar(userStore.user.id)
    userStore.setAvatar(blob)
  }
}

onMounted(() => {
  loadAvatar()
  checkLayout()
  window.addEventListener('resize', checkLayout)
})
</script>

<style scoped>
/* ===== CSS Variables ===== */
.home-container {
  --bg-primary: #ffffff;
  --bg-secondary: #f5f7fa;
  --bg-tertiary: #fafafa;
  --text-primary: #1f2937;
  --text-secondary: #6b7280;
  --text-muted: #9ca3af;
  --border-color: #d1d5db;
  --border-light: rgba(0, 0, 0, 0.06);
  --sidebar-bg: linear-gradient(180deg, #ffffff 0%, #fafafa 100%);
  --sidebar-glow: rgba(14, 165, 233, 0.08);
  --nav-item-color: #6b7280;
  --nav-item-hover: #374151;
  --nav-item-active: #0284c7;
  --header-bg: rgba(255, 255, 255, 0.9);
  --dropdown-bg: #ffffff;
  --action-btn-bg: #f3f4f6;
  --action-btn-hover: #e5e7eb;
  --card-bg: rgba(255, 255, 255, 0.03);
  --card-border: rgba(0, 0, 0, 0.05);
  --scrollbar-thumb: rgba(0, 0, 0, 0.15);
  --icon-color: #6b7280;
  --mobile-nav-bg: rgba(255, 255, 255, 0.95);
  --mobile-header-bg: rgba(255, 255, 255, 0.98);
}

.home-container.dark {
  --bg-primary: #0f0f12;
  --bg-secondary: #141418;
  --bg-tertiary: #1a1a1f;
  --text-primary: #fafafa;
  --text-secondary: #a1a1aa;
  --text-muted: #71717a;
  --border-color: rgba(255, 255, 255, 0.06);
  --border-light: rgba(255, 255, 255, 0.06);
  --sidebar-bg: linear-gradient(180deg, #1a1a1f 0%, #141418 100%);
  --sidebar-glow: rgba(14, 165, 233, 0.15);
  --nav-item-color: #a1a1aa;
  --nav-item-hover: #e4e4e7;
  --nav-item-active: #38bdf8;
  --header-bg: rgba(20, 20, 24, 0.8);
  --dropdown-bg: #1a1a1f;
  --action-btn-bg: rgba(255, 255, 255, 0.05);
  --action-btn-hover: rgba(255, 255, 255, 0.1);
  --card-bg: rgba(255, 255, 255, 0.03);
  --card-border: rgba(255, 255, 255, 0.05);
  --scrollbar-thumb: rgba(255, 255, 255, 0.1);
  --icon-color: #a1a1aa;
  --mobile-nav-bg: rgba(15, 15, 18, 0.95);
  --mobile-header-bg: rgba(15, 15, 18, 0.98);
}

/* ===== Base Layout ===== */
.home-container {
  display: flex;
  width: 100vw;
  height: 100vh;
  background: var(--bg-primary);
}

/* ===== PC Sidebar ===== */
.sidebar {
  width: 260px;
  min-width: 260px;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--sidebar-bg);
  border-right: 1px solid var(--border-light);
  position: relative;
  overflow: hidden;
}

.sidebar::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 200px;
  background: linear-gradient(180deg, var(--sidebar-glow) 0%, transparent 100%);
  pointer-events: none;
}

.sidebar-header {
  padding: 24px 20px;
  border-bottom: 1px solid var(--border-light);
  position: relative;
  z-index: 1;
}

.logo {
  display: flex;
  align-items: center;
  gap: 12px;
}

.logo-icon {
  width: 40px;
  height: 40px;
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 12px rgba(14, 165, 233, 0.3);
}

.logo-icon svg {
  width: 24px;
  height: 24px;
  color: white;
}

.logo-image {
  width: 40px;
  height: 40px;
  object-fit: contain;
  border-radius: 12px;
}

.logo-text {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.5px;
}

.sidebar-nav {
  flex: 1;
  padding: 20px 12px;
  overflow-y: auto;
  position: relative;
  z-index: 1;
}

.nav-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-divider-wrapper {
  padding: 8px 0 4px;
}

.nav-divider {
  height: 1px;
  background: var(--border-color);
  margin: 0 16px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  border-radius: 12px;
  color: var(--nav-item-color);
  text-decoration: none;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
}

.nav-item::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 0;
  background: linear-gradient(180deg, #0284c7 0%, #0ea5e9 100%);
  border-radius: 0 2px 2px 0;
  transition: height 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.home-container.dark .nav-item:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--nav-item-hover);
}

.home-container:not(.dark) .nav-item:hover {
  background: rgba(0, 0, 0, 0.04);
  color: var(--nav-item-hover);
}

.nav-item.active {
  background: rgba(14, 165, 233, 0.15);
  color: var(--nav-item-active);
}

.nav-item.active::before {
  height: 24px;
}

.nav-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  transition: transform 0.25s ease;
}

.nav-item:hover .nav-icon {
  transform: scale(1.1);
}

.nav-item.active .nav-icon {
  color: var(--nav-item-active);
}

.nav-label {
  font-size: 15px;
  font-weight: 500;
  letter-spacing: -0.2px;
}

.nav-indicator {
  position: absolute;
  right: 16px;
  width: 6px;
  height: 6px;
  background: #0ea5e9;
  border-radius: 50%;
  box-shadow: 0 0 8px rgba(14, 165, 233, 0.6);
}

/* ===== Main Content ===== */
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-primary);
}

/* ===== PC Header ===== */
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 32px;
  background: var(--header-bg);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--border-light);
  position: sticky;
  top: 0;
  z-index: 10;
}

.header-left {
  display: flex;
  align-items: center;
}

.page-title {
  font-size: 24px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
  letter-spacing: -0.5px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.action-btn {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--action-btn-bg);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s ease;
  color: var(--text-secondary);
}

.action-btn:hover {
  background: var(--action-btn-hover);
  color: var(--text-primary);
}

.action-icon {
  width: 20px;
  height: 20px;
  color: var(--icon-color);
}

.user-menu {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 16px 8px 8px;
  background: var(--action-btn-bg);
  border-radius: 40px;
  cursor: pointer;
  transition: all 0.25s ease;
  border: 1px solid var(--border-light);
}

.user-menu:hover {
  background: var(--action-btn-hover);
}

.header-avatar {
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  font-weight: 600;
}

.header-username {
  color: var(--text-primary);
  font-size: 14px;
  font-weight: 500;
}

.dropdown-arrow {
  color: var(--text-muted);
  margin-left: 4px;
}

/* ===== Content Area ===== */
.content-area {
  flex: 1;
  overflow: hidden;
  padding: 0 12px;
  display: flex;
  flex-direction: column;
}

.content-area::-webkit-scrollbar {
  width: 8px;
}

.content-area::-webkit-scrollbar-track {
  background: transparent;
}

.content-area::-webkit-scrollbar-thumb {
  background: var(--scrollbar-thumb);
  border-radius: 4px;
}

.content-area::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}

.route-loading {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.route-loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-light);
  border-top-color: var(--nav-item-active);
  border-radius: 50%;
  animation: route-spin 0.8s linear infinite;
}

@keyframes route-spin {
  to { transform: rotate(360deg); }
}

/* ===== Mobile Layout ===== */
.mobile-layout .main-content {
  padding-bottom: 64px;
}

.mobile-header {
  background: var(--mobile-header-bg);
  backdrop-filter: blur(12px);
  border-bottom: 1px solid var(--border-light);
  position: sticky;
  top: 0;
  z-index: 10;
}

.mobile-header-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  min-height: 56px;
}

.mobile-avatar {
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  font-weight: 600;
  font-size: 14px;
  cursor: pointer;
  flex-shrink: 0;
}

.mobile-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
  text-align: center;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 0 12px;
}

.mobile-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.mobile-action-btn {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--action-btn-bg);
  border-radius: 10px;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.2s ease;
}

.mobile-action-btn:active {
  transform: scale(0.95);
}

/* ===== Mobile Bottom Navigation ===== */
.mobile-nav {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: 64px;
  background: var(--mobile-nav-bg);
  backdrop-filter: blur(12px);
  border-top: 1px solid var(--border-light);
  display: flex;
  align-items: center;
  justify-content: space-around;
  padding: 0 8px;
  padding-bottom: env(safe-area-inset-bottom, 0);
  z-index: 100;
}

.mobile-nav-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 8px 12px;
  border-radius: 12px;
  color: var(--text-muted);
  text-decoration: none;
  transition: all 0.2s ease;
  min-width: 48px;
}

.mobile-nav-item.active {
  color: var(--nav-item-active);
  background: rgba(14, 165, 233, 0.12);
}

.mobile-nav-item:active {
  transform: scale(0.95);
}

.mobile-nav-label {
  font-size: 11px;
  font-weight: 500;
}

.mobile-nav-item.active .mobile-nav-label {
  font-weight: 600;
}

/* ===== Mobile User Drawer ===== */
.mobile-user-drawer {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
}

.drawer-header {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 24px 20px;
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  margin-bottom: 8px;
}

.drawer-avatar {
  background: rgba(255, 255, 255, 0.2);
  font-weight: 600;
  font-size: 22px;
  border: 2px solid rgba(255, 255, 255, 0.3);
}

.drawer-user-info {
  flex: 1;
}

.drawer-username {
  font-size: 18px;
  font-weight: 600;
  color: #ffffff;
  margin-bottom: 4px;
}

.drawer-system {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.8);
}

.drawer-menu {
  padding: 8px;
}

.drawer-menu-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  border-radius: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.2s ease;
}

.drawer-menu-item:hover {
  background: var(--action-btn-bg);
  color: var(--text-primary);
}

.drawer-menu-item:active {
  background: var(--action-btn-hover);
}

.drawer-divider {
  height: 1px;
  background: var(--border-light);
  margin: 8px 16px;
}

.lang-item {
  justify-content: flex-start;
}

.lang-options {
  margin-left: auto;
  display: flex;
  gap: 8px;
}

.lang-options span {
  padding: 4px 12px;
  border-radius: 16px;
  font-size: 13px;
  background: var(--action-btn-bg);
  transition: all 0.2s ease;
}

.lang-options span.active {
  background: linear-gradient(135deg, #0284c7 0%, #0ea5e9 100%);
  color: #ffffff;
}

.logout-item {
  color: #ef4444;
}

.logout-item:hover {
  background: rgba(239, 68, 68, 0.1);
}

/* ===== Mobile Content Area Adjustment ===== */
.mobile-layout .content-area {
  padding: 0 8px;
}

/* ===== Mobile More Menu ===== */
.more-btn {
  cursor: pointer;
}

.more-menu-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 200;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}

.more-menu-popup {
  background: var(--bg-primary);
  border-radius: 20px 20px 0 0;
  padding: 20px;
  padding-bottom: calc(20px + env(safe-area-inset-bottom, 0));
  width: 100%;
  max-width: 400px;
  box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.15);
}

.more-menu-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}

.more-menu-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 16px 8px;
  border-radius: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.2s ease;
}

.more-menu-item:active {
  transform: scale(0.95);
  background: var(--action-btn-bg);
}

.more-menu-item.active {
  color: var(--nav-item-active);
  background: rgba(14, 165, 233, 0.12);
}

.more-menu-item span {
  font-size: 12px;
  font-weight: 500;
}

/* More Menu Transition */
.more-menu-fade-enter-active,
.more-menu-fade-leave-active {
  transition: all 0.3s ease;
}

.more-menu-fade-enter-active .more-menu-popup,
.more-menu-fade-leave-active .more-menu-popup {
  transition: transform 0.3s ease;
}

.more-menu-fade-enter-from,
.more-menu-fade-leave-to {
  background: transparent;
}

.more-menu-fade-enter-from .more-menu-popup,
.more-menu-fade-leave-to .more-menu-popup {
  transform: translateY(100%);
}
</style>

<style>
.el-drawer__body:has(.mobile-user-drawer) { padding: 0 !important; }

.custom-dropdown {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 12px;
  padding: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.15);
}

.custom-dropdown .el-dropdown-menu__item {
  color: var(--el-text-color-secondary);
  border-radius: 8px;
  padding: 10px 16px;
  display: flex;
  align-items: center;
  gap: 10px;
  transition: all 0.2s ease;
}
</style>
