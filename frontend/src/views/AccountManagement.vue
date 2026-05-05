<template>
  <div class="account-management-container">
    <div class="account-toolbar">
      <div class="toolbar-left">
        <el-button type="primary" @click="handleAddUser">
          <el-icon><Plus /></el-icon>
          <span class="btn-text">{{ t('accountManagement.addUser') }}</span>
        </el-button>
      </div>
    </div>

    <div class="table-wrapper">
      <el-table
        :data="userList"
        style="width: 100%"
        stripe
        border
        height="100%"
        show-overflow-tooltip
        v-loading="loading"
      >
        <el-table-column :label="t('accountManagement.username')" min-width="150" prop="username" />
        
        <el-table-column :label="t('accountManagement.rootPath')" min-width="200" prop="root_path">
          <template #default="{ row }">
            {{ row.root_path || '-' }}
          </template>
        </el-table-column>
        
        <el-table-column :label="t('accountManagement.isAdmin')" width="120" prop="is_admin">
          <template #default="{ row }">
            <el-tag :type="row.is_admin ? 'success' : 'info'" size="small">
              {{ row.is_admin ? t('accountManagement.yes') : t('accountManagement.no') }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column :label="t('accountManagement.expireTime')" width="180" prop="expire_at">
          <template #default="{ row }">
            {{ formatExpireTime(row.expire_at) }}
          </template>
        </el-table-column>
        
        <el-table-column :label="t('accountManagement.remark')" min-width="200" prop="remark">
          <template #default="{ row }">
            <span class="remark-text">{{ row.remark || '-' }}</span>
          </template>
        </el-table-column>
        
        <el-table-column :label="t('accountManagement.operations')" width="150" fixed="right">
          <template #default="{ row }">
            <el-link type="primary" @click="handleEdit(row)">
              <el-icon><Edit /></el-icon>
              {{ t('accountManagement.edit') }}
            </el-link>
            <el-link type="danger" @click="handleDelete(row)">
              <el-icon><Delete /></el-icon>
              {{ t('accountManagement.delete') }}
            </el-link>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <el-drawer
      v-model="drawerVisible"
      :title="isEdit ? t('accountManagement.editUser') : t('accountManagement.addUser')"
      direction="rtl"
      size="450px"
    >
      <el-form
        ref="formRef"
        :model="formData"
        :rules="formRules"
        label-width="100px"
        label-position="left"
        @submit.prevent
      >
        <el-form-item :label="t('accountManagement.username')" prop="username">
          <el-input
            v-model="formData.username"
            :placeholder="t('accountManagement.usernamePlaceholder')"
            :disabled="isEdit"
          />
        </el-form-item>
        
        <el-form-item :label="t('accountManagement.password')" prop="password">
          <el-input
            v-model="formData.password"
            type="password"
            show-password
            :placeholder="isEdit ? t('accountManagement.passwordEditPlaceholder') : t('accountManagement.passwordPlaceholder')"
          />
        </el-form-item>
        
        <el-form-item :label="t('accountManagement.rootPath')" prop="root_path">
          <el-input
            v-model="formData.root_path"
            :placeholder="t('accountManagement.rootPathPlaceholder')"
          />
        </el-form-item>
        
        <el-form-item :label="t('accountManagement.recycleBinPath')" prop="recycle_bin_path">
          <el-input
            v-model="formData.recycle_bin_path"
            :placeholder="t('accountManagement.recycleBinPathPlaceholder')"
            clearable
          />
        </el-form-item>
        
        <el-form-item :label="t('accountManagement.isAdmin')" prop="is_admin">
          <el-switch v-model="formData.is_admin" />
        </el-form-item>
        
        <el-form-item :label="t('accountManagement.expireTime')" prop="expire_at">
          <el-date-picker
            v-model="expireAtDate"
            type="datetime"
            :placeholder="t('accountManagement.expireTimePlaceholder')"
            format="YYYY-MM-DD HH:mm:ss"
            style="width: 100%"
            :clearable="true"
          />
        </el-form-item>
        
        <el-form-item :label="t('accountManagement.remark')" prop="remark">
          <el-input
            v-model="formData.remark"
            type="textarea"
            :rows="3"
            :placeholder="t('accountManagement.remarkPlaceholder')"
          />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="drawerVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="submitting" @click="handleSubmit">{{ t('common.confirm') }}</el-button>
      </template>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox } from 'element-plus'
import { Plus, Edit, Delete } from '@element-plus/icons-vue'
import type { FormInstance, FormRules } from 'element-plus'
import { getUserList, createUser, updateUser, deleteUser, type UserItem, type CreateUserRequest, type UpdateUserRequest } from '@/api/system'

const { t } = useI18n()

const formRef = ref<FormInstance>()
const drawerVisible = ref(false)
const isEdit = ref(false)
const loading = ref(false)
const submitting = ref(false)
const editingUserId = ref<string>('')

interface FormData {
  username: string
  password: string
  root_path: string
  recycle_bin_path: string
  is_admin: boolean
  expire_at: string
  remark: string
}

const formData = reactive<FormData>({
  username: '',
  password: '',
  root_path: '',
  recycle_bin_path: '',
  is_admin: false,
  expire_at: '',
  remark: '',
})

const formRules = computed<FormRules>(() => ({
  username: [
    { required: true, message: t('accountManagement.usernameRequired'), trigger: 'blur' },
  ],
  password: [
    { required: !isEdit.value, message: t('accountManagement.passwordRequired'), trigger: 'blur' },
  ],
}))

const expireAtDate = computed({
  get: () => {
    if (!formData.expire_at) return null
    return new Date(formData.expire_at)
  },
  set: (val: Date | null) => {
    if (val) {
      formData.expire_at = val.toISOString()
    } else {
      formData.expire_at = ''
    }
  }
})

const userList = ref<UserItem[]>([])

const fetchUserList = async () => {
  loading.value = true
  try {
    userList.value = await getUserList()
  } catch {
  } finally {
    loading.value = false
  }
}

const formatExpireTime = (expireAt: string | null) => {
  if (!expireAt) return t('accountManagement.permanent')
  const date = new Date(expireAt)
  return date.toLocaleString()
}

const resetForm = () => {
  formData.username = ''
  formData.password = ''
  formData.root_path = ''
  formData.recycle_bin_path = ''
  formData.is_admin = false
  formData.expire_at = ''
  formData.remark = ''
  editingUserId.value = ''
  expireAtDate.value = null
}

const handleAddUser = () => {
  isEdit.value = false
  resetForm()
  drawerVisible.value = true
}

const handleEdit = (row: UserItem) => {
  isEdit.value = true
  editingUserId.value = row.id
  formData.username = row.username
  formData.password = ''
  formData.root_path = row.root_path || ''
      formData.recycle_bin_path = row.recycle_bin_path || ''
  formData.is_admin = row.is_admin
  formData.expire_at = row.expire_at || ''
  formData.remark = row.remark || ''
  drawerVisible.value = true
}

const handleDelete = async (row: UserItem) => {
  try {
    await ElMessageBox.confirm(
      t('accountManagement.deleteConfirm', { name: row.username }),
      t('common.confirm'),
      { type: 'warning' }
    )
  } catch {
    return
  }
  try {
    await deleteUser(row.id)
    ElMessage.success({ __key: 'accountManagement.deleteSuccess' })
    fetchUserList()
  } catch {
  }
}

const handleSubmit = async () => {
  if (!formRef.value) return
  
  const valid = await formRef.value.validate().catch(() => false)
  if (!valid) return

  submitting.value = true
  try {
    if (isEdit.value) {
      const updateData: UpdateUserRequest = {
        id: editingUserId.value,
        root_path: formData.root_path || '',
        recycle_bin_path: formData.recycle_bin_path || null,
        is_admin: formData.is_admin,
        expire_at: formData.expire_at || '',
        remark: formData.remark || '',
      }
      if (formData.password) {
        updateData.password = formData.password
      }
      await updateUser(updateData)
      ElMessage.success({ __key: 'accountManagement.editSuccess' })
    } else {
      const createData: CreateUserRequest = {
        username: formData.username,
        password: formData.password,
      }
      if (formData.root_path) {
        createData.root_path = formData.root_path
      }
      if (formData.recycle_bin_path) {
        createData.recycle_bin_path = formData.recycle_bin_path
      }
      createData.is_admin = formData.is_admin
      if (formData.expire_at) {
        createData.expire_at = formData.expire_at
      }
      if (formData.remark) {
        createData.remark = formData.remark
      }
      await createUser(createData)
      ElMessage.success({ __key: 'accountManagement.addSuccess' })
    }
    drawerVisible.value = false
    fetchUserList()
  } catch {
  } finally {
    submitting.value = false
  }
}

onMounted(() => {
  fetchUserList()
})
</script>

<style scoped>
.account-management-container {
  padding: 20px;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.account-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  flex-shrink: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
}

.table-wrapper {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.btn-text {
  margin-left: 6px;
}

.remark-text {
  color: var(--el-text-color-secondary);
}
</style>
