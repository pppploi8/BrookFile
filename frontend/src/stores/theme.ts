import { defineStore } from 'pinia'
import { ref } from 'vue'

export type Theme = 'light' | 'dark'

const getInitialTheme = (): Theme => {
  const saved = localStorage.getItem('theme')
  return (saved === 'light' || saved === 'dark') ? saved : 'dark'
}

const initialTheme = getInitialTheme()

const applyThemeToDom = (newTheme: Theme) => {
  const root = document.documentElement
  if (newTheme === 'dark') {
    root.classList.add('dark')
  } else {
    root.classList.remove('dark')
  }
}

if (typeof document !== 'undefined') {
  applyThemeToDom(initialTheme)
}

export const useThemeStore = defineStore('theme', () => {
  const theme = ref<Theme>(initialTheme)
  const isDark = ref(initialTheme === 'dark')

  const setTheme = (newTheme: Theme) => {
    theme.value = newTheme
    isDark.value = newTheme === 'dark'
    applyTheme(newTheme)
    localStorage.setItem('theme', newTheme)
  }

  const toggleTheme = () => {
    setTheme(theme.value === 'dark' ? 'light' : 'dark')
  }

  const applyTheme = (newTheme: Theme) => {
    applyThemeToDom(newTheme)
  }

  const resetToLight = () => {
    theme.value = 'light'
    isDark.value = false
    applyThemeToDom('light')
  }

  const initTheme = () => {
    const savedTheme = localStorage.getItem('theme') as Theme | null
    const targetTheme = savedTheme || 'dark'
    theme.value = targetTheme
    isDark.value = targetTheme === 'dark'
    applyThemeToDom(targetTheme)
  }

  return {
    theme,
    isDark,
    setTheme,
    toggleTheme,
    initTheme,
    resetToLight
  }
})
