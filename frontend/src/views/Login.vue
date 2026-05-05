<template>
  <div class="login-page">
    <div class="login-card">
      <div class="login-header">
        <div class="logo">
          <img v-if="userStore.systemLogoUrl" :src="userStore.systemLogoUrl" class="logo-image" alt="Logo" />
          <svg v-else width="40" height="40" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M3 7V17C3 18.1046 3.89543 19 5 19H19C20.1046 19 21 18.1046 21 17V9C21 7.89543 20.1046 7 19 7H13L11 5H5C3.89543 5 3 5.89543 3 7Z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <h1>{{ userStore.systemName }}</h1>
      </div>
      
      <div class="login-body">
        <div class="form-title">{{ t('login.title') }}</div>
        
        <el-form ref="formRef" :model="form" :rules="rules" label-position="top" @submit.prevent="handleLogin">
          <el-form-item prop="username" :label="t('login.username')">
            <el-input 
              v-model="form.username" 
              :placeholder="t('login.usernamePlaceholder')"
              autocomplete="username"
            />
          </el-form-item>
          
          <el-form-item prop="password" :label="t('login.password')">
            <el-input 
              v-model="form.password" 
              type="password"
              :placeholder="t('login.passwordPlaceholder')"
              autocomplete="current-password"
            />
          </el-form-item>
          
          <el-form-item>
            <el-button type="primary" class="login-btn" :loading="loading" native-type="submit">
              {{ loading ? t('login.loading') : t('login.submit') }}
            </el-button>
          </el-form-item>
        </el-form>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { type FormInstance, type FormRules } from 'element-plus'
import { login, getSystemInfo } from '@/api'
import { useUserStore } from '@/stores/user'

const { t } = useI18n()
const router = useRouter()
const userStore = useUserStore()

const formRef = ref<FormInstance>()
const loading = ref(false)

const form = reactive({
  username: '',
  password: '',
})

const rules = reactive<FormRules>({
  username: [
    { required: true, message: t('login.pleaseEnterUsername'), trigger: 'blur' },
  ],
  password: [
    { required: true, message: t('login.pleaseEnterPassword'), trigger: 'blur' },
  ],
})

const handleLogin = async () => {
  if (!formRef.value) return
  
  loading.value = true
  try {
    const valid = await formRef.value.validate()
    if (!valid) return
    
    await login({
      username: form.username,
      password: form.password,
    })
    const systemInfo = await getSystemInfo(true)
    userStore.setSystemInfo(systemInfo)
    ElMessage.success({ __key: 'login.success' })
    router.push('/')
  } catch {
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
  max-width: 380px;
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

.login-btn {
  width: 100%;
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
}
</style>
