import { createApp } from 'vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'

import App from './App.vue'
import router from './router'
import pinia from './stores'
import i18n from './i18n'
import './style.css'

const app = createApp(App)

for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

app.use(ElementPlus)
app.use(router)
app.use(pinia)
app.use(i18n)

app.mount('#app')

if (import.meta.env.DEV) {
  fetch('/vue-agent.js').then(r => r.ok ? r.text() : null).then(code => {
    if (code) {
      const s = document.createElement('script')
      s.textContent = code
      document.head.appendChild(s)
    }
  })
}
