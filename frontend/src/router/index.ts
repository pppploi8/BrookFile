import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import { useUserStore } from '@/stores/user'

const routes: RouteRecordRaw[] = [
  {
    path: '/init',
    name: 'Init',
    component: () => import('@/views/Init.vue'),
    meta: { requiresNoInit: true },
  },
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/Login.vue'),
    meta: { requiresInit: true, requiresNoLogin: true },
  },
  {
    path: '/',
    name: 'Home',
    component: () => import('@/views/Home.vue'),
    meta: { requiresAuth: true },
    redirect: '/files',
    children: [
      {
        path: 'files',
        name: 'Files',
        component: () => import('@/views/categories/Files.vue'),
      },
      {
        path: 'notes',
        name: 'Notes',
        component: () => import('@/views/categories/Notes.vue'),
      },
      {
        path: 'passwords',
        name: 'Passwords',
        component: () => import('@/views/categories/Passwords.vue'),
      },
      {
        path: 'recycle-bin',
        name: 'RecycleBin',
        component: () => import('@/views/categories/RecycleBin.vue'),
      },
      {
        path: 'shares',
        name: 'Shares',
        component: () => import('@/views/categories/Shares.vue'),
      },
      {
        path: 'account-management',
        name: 'AccountManagement',
        component: () => import('@/views/AccountManagement.vue'),
      },
      {
        path: 'profile',
        name: 'ProfileCenter',
        component: () => import('@/views/ProfileCenter.vue'),
      },
      {
        path: 'settings',
        name: 'Settings',
        component: () => import('@/views/Settings.vue'),
      },
    ],
  },
  {
    path: '/s/:code',
    name: 'SharePage',
    component: () => import('@/views/SharePage.vue'),
    meta: { public: true },
  },
  { path: '/:pathMatch(.*)*', redirect: '/' },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

router.beforeEach((to) => {
  const userStore = useUserStore()

  if (to.meta.requiresInit && !userStore.initialized) {
    return { path: '/init', replace: true }
  }

  if (to.meta.requiresNoInit && userStore.initialized) {
    return { path: '/', replace: true }
  }

  if (to.meta.requiresAuth && !userStore.loggedIn) {
    return { path: '/login', replace: true }
  }

  if (to.meta.requiresNoLogin && userStore.loggedIn) {
    return { path: '/', replace: true }
  }
})

export default router
