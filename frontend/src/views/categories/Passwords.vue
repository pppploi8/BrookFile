<template>
  <div class="passwords-container" @click="hideContextMenu" @contextmenu="hideContextMenu">
    <!-- ===== Mobile View ===== -->
    <template v-if="isMobile">
      <div v-if="!currentVault" class="mobile-vault-list">
        <div
          v-for="vault in vaults"
          :key="vault.id"
          class="mobile-vault-card"
          @click="selectVault(vault)"
        >
          <div class="mobile-vault-icon">
            <el-icon v-if="unlockedVaultIds.has(vault.id)" :size="22"><Unlock /></el-icon>
            <el-icon v-else :size="22"><Lock /></el-icon>
          </div>
          <div class="mobile-vault-info">
            <div class="mobile-vault-name">{{ vault.name }}</div>
            <div v-if="vault.description" class="mobile-vault-desc">{{ vault.description }}</div>
          </div>
          <el-icon class="mobile-vault-arrow"><ArrowRight /></el-icon>
        </div>
        <div v-if="vaults.length === 0" class="mobile-empty">
          {{ t('passwords.noVaults') }}
        </div>
      </div>

      <div v-else class="mobile-pwd-view">
        <div class="mobile-pwd-toolbar">
          <div class="mobile-pwd-back" @click="currentVault = null">
            <el-icon><ArrowLeft /></el-icon>
            <span>{{ currentVault.name }}</span>
          </div>
          <el-button type="primary" size="small" @click="handleAddPassword">
            <el-icon><Plus /></el-icon>
            {{ t('common.add') }}
          </el-button>
        </div>
        <el-input
          v-model="searchKeyword"
          :placeholder="t('passwords.searchPlaceholder')"
          clearable
          class="mobile-pwd-search"
          :prefix-icon="Search"
        />
        <div class="mobile-pwd-list">
          <div v-if="filteredPasswords.length === 0" class="mobile-empty">
            {{ t('passwords.noPasswords') }}
          </div>
          <div
            v-for="item in filteredPasswords"
            :key="item.id"
            class="mobile-pwd-card"
          >
            <div class="mobile-pwd-title">{{ item.title }}</div>
            <div class="mobile-pwd-row" @click="copyText(item.username)">
              <span class="mobile-pwd-label">{{ t('passwords.username') }}</span>
              <span class="mobile-pwd-value">{{ item.username }}</span>
              <el-icon class="mobile-pwd-copy"><CopyDocument /></el-icon>
            </div>
            <div class="mobile-pwd-row" @click="copyText(item.password)">
              <span class="mobile-pwd-label">{{ t('passwords.password') }}</span>
              <span class="mobile-pwd-value">••••••••</span>
              <el-icon class="mobile-pwd-copy"><CopyDocument /></el-icon>
            </div>
            <div v-if="item.remark" class="mobile-pwd-remark">{{ item.remark }}</div>
            <div class="mobile-pwd-actions">
              <el-link type="primary" @click="handleEditPassword(item)">
                <el-icon><Edit /></el-icon>
                {{ t('passwords.edit') }}
              </el-link>
              <el-link type="danger" @click="handleDeletePassword(item)">
                <el-icon><Delete /></el-icon>
                {{ t('passwords.delete') }}
              </el-link>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- ===== Desktop View ===== -->
    <template v-else>
      <div class="vault-panel">
  <el-tree
    :data="treeData"
    :props="treeProps"
    :current-node-key="currentTreeNodeKey"
    highlight-current
    node-key="id"
    default-expand-all
    :expand-on-click-node="false"
    @contextmenu.prevent
    @node-click="handleNodeClick"
    @node-contextmenu="handleNodeContextMenu"
  >
    <template #default="{ data }">
      <span class="tree-node">
        <el-icon v-if="data.isRoot"><Key /></el-icon>
        <el-icon v-else-if="data.isFolder" class="tree-node-folder"><Folder /></el-icon>
        <el-icon v-else-if="unlockedVaultIds.has(data.id)" class="tree-node-unlocked"><Unlock /></el-icon>
        <el-icon v-else class="tree-node-locked"><Lock /></el-icon>
        <span>{{ data.label }}</span>
      </span>
    </template>
  </el-tree>
      </div>

      <div v-if="currentVault" class="password-panel">
        <div class="password-toolbar">
          <el-button type="primary" @click="handleAddPassword">
            <el-icon><Plus /></el-icon>
            <span class="btn-text">{{ t('passwords.addPassword') }}</span>
          </el-button>
          <el-input
            v-model="searchKeyword"
            class="search-input"
            :placeholder="t('passwords.searchPlaceholder')"
            :prefix-icon="Search"
            clearable
          />
        </div>
        <div class="table-wrapper">
          <el-table :data="filteredPasswords" stripe border height="100%" show-overflow-tooltip>
            <el-table-column :label="t('passwords.title')" min-width="200" prop="title" />
            <el-table-column :label="t('passwords.username')" min-width="180" prop="username" />
            <el-table-column :label="t('passwords.password')" min-width="150">
              <template #default>
                <span>••••••••</span>
              </template>
            </el-table-column>
            <el-table-column :label="t('passwords.remark')" min-width="200" prop="remark">
              <template #default="{ row }">{{ row.remark || '-' }}</template>
            </el-table-column>
            <el-table-column :label="t('passwords.operations')" width="280" fixed="right">
              <template #default="{ row }">
                <el-link type="primary" @click="handleEditPassword(row)">
                  <el-icon><Edit /></el-icon>
                  {{ t('passwords.edit') }}
                </el-link>
                <el-link type="primary" @click="copyText(row.username)">
                  <el-icon><CopyDocument /></el-icon>
                  {{ t('passwords.copyUsername') }}
                </el-link>
                <el-link type="primary" @click="copyText(row.password)">
                  <el-icon><CopyDocument /></el-icon>
                  {{ t('passwords.copyPassword') }}
                </el-link>
                <el-link type="danger" @click="handleDeletePassword(row)">
                  <el-icon><Delete /></el-icon>
                  {{ t('passwords.delete') }}
                </el-link>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </div>

      <div v-else class="password-empty">
        <el-icon :size="48" color="var(--el-text-color-secondary)"><Key /></el-icon>
        <p>{{ t('passwords.selectVaultHint') }}</p>
      </div>
    </template>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.visible"
      class="context-menu"
      :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
      @click.stop
      @contextmenu.stop
    >
      <template v-if="contextMenu.isRoot">
        <div class="context-menu-item" @click="handleCreateVault">
          <el-icon><Plus /></el-icon>
          {{ t('passwords.create') }}
        </div>
        <div class="context-menu-item" @click="handleImportVault">
          <el-icon><Upload /></el-icon>
          {{ t('passwords.import') }}
        </div>
      </template>
      <template v-else-if="contextMenu.isFolder">
        <div class="context-menu-item" @click="handleCreateFolder">
          <el-icon><Plus /></el-icon>
          {{ t('passwords.create') }}
        </div>
        <div class="context-menu-item" @click="handleRenameFolder">
          <el-icon><Edit /></el-icon>
          {{ t('common.rename') }}
        </div>
        <div class="context-menu-item context-menu-danger" @click="handleDeleteFolder">
          <el-icon><Delete /></el-icon>
          {{ t('passwords.delete') }}
        </div>
      </template>
      <template v-else>
        <div class="context-menu-item" @click="handleEditVault">
          <el-icon><Edit /></el-icon>
          {{ t('passwords.edit') }}
        </div>
        <div class="context-menu-item context-menu-danger" @click="handleDeleteVault">
          <el-icon><Delete /></el-icon>
          {{ t('passwords.delete') }}
        </div>
        <template v-if="unlockedVaultIds.has(contextMenu.vaultId)">
          <div class="context-menu-divider" />
          <div class="context-menu-item" @click="handleCreateFolder">
            <el-icon><Plus /></el-icon>
            {{ t('passwords.createFolder') }}
          </div>
          <div class="context-menu-item" @click="handleLockVault">
            <el-icon><Lock /></el-icon>
            {{ t('passwords.lock') }}
          </div>
        </template>
      </template>
    </div>

    <!-- Create Vault Dialog -->
    <el-dialog v-model="createVaultVisible" :title="t('passwords.createVault')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form ref="createVaultFormRef" :model="createVaultForm" :rules="createVaultRules" label-position="top" @submit.prevent>
        <el-form-item :label="t('passwords.vaultName')" prop="name">
          <el-input v-model="createVaultForm.name" :placeholder="t('passwords.vaultNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultPath')" prop="path">
          <FolderSelect v-model="createVaultForm.path" :placeholder="t('passwords.vaultPathPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultFilename')" prop="filename">
          <el-input v-model="createVaultForm.filename" :placeholder="t('passwords.vaultFilenamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultDescription')">
          <el-input v-model="createVaultForm.description" type="textarea" :rows="3" :placeholder="t('passwords.vaultDescriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.masterPassword')" prop="password">
          <el-input v-model="createVaultForm.password" type="password" show-password :placeholder="t('passwords.masterPasswordPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.encryptRounds')">
          <div class="encrypt-rounds-wrapper">
            <el-slider v-model="createVaultForm.rounds" :min="1" :max="10000000" :step="1" :show-tooltip="true" :format-tooltip="(val: number) => val.toLocaleString()" />
            <el-button size="small" @click="testEncryptRounds('create')" :loading="isTestingCreateRounds">
              {{ isTestingCreateRounds ? t('passwords.encryptRoundsTesting') : t('passwords.encryptRoundsTest') }}
            </el-button>
            <div v-if="testCreateRoundsResult !== null" class="encrypt-rounds-result">
              {{ t('passwords.encryptRoundsResult', { ms: testCreateRoundsResult }) }}
            </div>
            <div class="encrypt-rounds-hint">{{ t('passwords.encryptRoundsHint') }}</div>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createVaultVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="isCreatingVault" @click="submitCreateVault">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Import Vault Dialog -->
    <el-dialog v-model="importVaultVisible" :title="t('passwords.importVault')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form ref="importVaultFormRef" :model="importVaultForm" :rules="importVaultRules" label-position="top" @submit.prevent>
        <el-form-item :label="t('passwords.vaultName')" prop="name">
          <el-input v-model="importVaultForm.name" :placeholder="t('passwords.vaultNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.importVaultFile')" prop="path">
          <FileSelect v-model="importVaultForm.path" :placeholder="t('passwords.importVaultFilePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultDescription')">
          <el-input v-model="importVaultForm.description" type="textarea" :rows="3" :placeholder="t('passwords.vaultDescriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.masterPassword')" prop="password">
          <el-input v-model="importVaultForm.password" type="password" show-password :placeholder="t('passwords.masterPasswordPlaceholder')" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="importVaultVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="isImportingVault" @click="submitImportVault">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Unlock Vault Dialog -->
    <el-dialog v-model="unlockDialogVisible" :title="t('passwords.unlockVault')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item :label="t('passwords.masterPassword')">
          <el-input v-model="unlockPassword" type="password" show-password :placeholder="t('passwords.masterPasswordPlaceholder')" @keyup.enter="submitUnlock" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="unlockDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="isUnlocking" @click="submitUnlock">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Edit Vault Dialog -->
    <el-dialog v-model="editVaultVisible" :title="t('passwords.editVault')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item :label="t('passwords.vaultName')">
          <el-input v-model="editVaultForm.name" :placeholder="t('passwords.vaultNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultPath')">
          <FolderSelect v-model="editVaultForm.path" :placeholder="t('passwords.vaultPathPlaceholder')" disabled />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultFilename')">
          <el-input v-model="editVaultForm.filename" disabled />
        </el-form-item>
        <el-form-item :label="t('passwords.vaultDescription')">
          <el-input v-model="editVaultForm.description" type="textarea" :rows="3" :placeholder="t('passwords.vaultDescriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.masterPassword')">
          <el-input v-model="editVaultForm.password" type="password" show-password :disabled="!editVaultForm.unlocked" :placeholder="editVaultForm.unlocked ? t('passwords.masterPasswordPlaceholderLeaveEmpty') : t('passwords.unlockRequired')" />
        </el-form-item>
        <el-form-item :label="t('passwords.encryptRounds')">
          <div class="encrypt-rounds-wrapper">
            <el-slider v-model="editVaultForm.rounds" :disabled="!editVaultForm.unlocked" :min="1" :max="10000000" :step="1" :show-tooltip="true" :format-tooltip="(val: number) => val.toLocaleString()" />
            <el-button size="small" :disabled="!editVaultForm.unlocked" @click="testEncryptRounds('edit')" :loading="isTestingEditRounds">
              {{ isTestingEditRounds ? t('passwords.encryptRoundsTesting') : t('passwords.encryptRoundsTest') }}
            </el-button>
            <div v-if="testEditRoundsResult !== null" class="encrypt-rounds-result">
              {{ t('passwords.encryptRoundsResult', { ms: testEditRoundsResult }) }}
            </div>
            <div class="encrypt-rounds-hint">{{ t('passwords.encryptRoundsHint') }}</div>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editVaultVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="isSavingVault" @click="submitEditVault">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="folderDialogVisible"
      :title="isEditingFolder ? t('passwords.renameFolder') : t('passwords.createFolder')"
      :width="isMobile ? '90%' : '420px'"
      destroy-on-close
    >
      <el-form label-position="top" @submit.prevent>
        <el-form-item :label="t('passwords.folderName')">
          <el-input v-model="folderFormName" :placeholder="t('passwords.folderNamePlaceholder')" @keyup.enter="submitFolderForm" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="folderDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="submitFolderForm">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <el-drawer
      v-model="passwordDialogVisible"
      :title="isEditPassword ? t('passwords.editPassword') : t('passwords.addPassword')"
      direction="rtl"
      size="360px"
      destroy-on-close
    >
      <el-form ref="passwordFormRef" :model="passwordForm" :rules="passwordRules" label-position="top" @submit.prevent>
        <el-form-item :label="t('passwords.title')" prop="title">
          <el-input v-model="passwordForm.title" :placeholder="t('passwords.titlePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.username')" prop="username">
          <el-input v-model="passwordForm.username" :placeholder="t('passwords.usernamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('passwords.password')" prop="password">
          <el-input v-model="passwordForm.password" type="password" show-password :placeholder="t('passwords.passwordPlaceholder')" />
        </el-form-item>
        <div class="pwd-generator">
          <div class="generator-header">{{ t('passwords.generator') }}</div>
          <div class="generator-row">
            <span class="generator-label">{{ t('passwords.generatorLength') }}</span>
            <el-input-number v-model="generatorConfig.length" :min="4" :max="128" size="small" style="width: 120px" />
          </div>
          <div class="generator-row">
            <span class="generator-label">{{ t('passwords.generatorChars') }}</span>
          </div>
          <div class="generator-checks">
            <el-checkbox v-model="generatorConfig.numbers">{{ t('passwords.generatorNumbers') }}</el-checkbox>
            <el-checkbox v-model="generatorConfig.lowercase">{{ t('passwords.generatorLowercase') }}</el-checkbox>
            <el-checkbox v-model="generatorConfig.uppercase">{{ t('passwords.generatorUppercase') }}</el-checkbox>
            <el-checkbox v-model="generatorConfig.special">{{ t('passwords.generatorSpecial') }}</el-checkbox>
          </div>
          <el-button size="small" style="width: 100%; margin-top: 8px" @click="generatePassword">
            <el-icon><Refresh /></el-icon>
            {{ t('passwords.generatorGenerate') }}
          </el-button>
        </div>
        <el-form-item :label="t('passwords.folder')">
          <el-select v-model="passwordForm.folder_id" :placeholder="t('passwords.folderPlaceholder')" clearable style="width: 100%">
            <el-option v-for="opt in passwordFolderOptions" :key="opt.id" :label="opt.label" :value="opt.id">
              <span :style="{ paddingLeft: opt.indent * 20 + 'px' }">{{ opt.label }}</span>
            </el-option>
          </el-select>
        </el-form-item>
        <el-form-item :label="t('passwords.remark')">
          <el-input v-model="passwordForm.remark" type="textarea" :rows="3" :placeholder="t('passwords.remarkPlaceholder')" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="passwordDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="isSavingPassword" @click="submitPasswordForm">{{ t('common.confirm') }}</el-button>
      </template>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox } from 'element-plus'
import {
  Search,
  Key,
  Plus,
  Edit,
  Delete,
  CopyDocument,
  Lock,
  Unlock,
  ArrowLeft,
  ArrowRight,
  Refresh,
  Upload,
  Folder,
} from '@element-plus/icons-vue'
import type { FormInstance, FormRules } from 'element-plus'
import FolderSelect from '@/components/FolderSelect.vue'
import FileSelect from '@/components/FileSelect.vue'
import {
  listVaults,
  createVault as createVaultApi,
  updateVault as updateVaultApi,
  updateVaultMeta as updateVaultMetaApi,
  deleteVault as deleteVaultApi,
  importVault as importVaultApi,
  uploadSingleFile,
  fetchDownloadFile,
  type VaultListItem,
} from '@/api/system'
import { backend, arrayBufferToBase64, base64ToArrayBuffer } from '@/stores/crypto'

const { t } = useI18n()

interface VaultItem {
  id: string
  name: string
  description: string
  path: string
  filename: string
}

interface PasswordItem {
  id: string
  folder_id: string | null
  title: string
  username: string
  password: string
  remark: string
  created_at: string
  updated_at: string
}

interface VaultData {
  folders: Array<{ id: string; name: string; parent_id: string | null }>
  passwords: PasswordItem[]
}

interface TreeNodeData {
  id: string
  label: string
  isRoot?: boolean
  isVault?: boolean
  isFolder?: boolean
  vaultId?: string
  children?: TreeNodeData[]
}

function generateId(): string {
  return crypto.randomUUID()
}

const isMobile = ref(false)
const currentVault = ref<VaultItem | null>(null)
const currentFolderId = ref<string | null>(null)
const searchKeyword = ref('')
const unlockedVaultIds = ref<Set<string>>(new Set())
const vaultMasterPasswords = ref<Map<string, string>>(new Map())

function encodePassword(pwd: string): string {
  try { return btoa(unescape(encodeURIComponent(pwd))) } catch { return btoa(pwd) }
}

function decodePassword(encoded: string): string {
  try { return decodeURIComponent(escape(atob(encoded))) } catch { return atob(encoded) }
}
const vaultKeyCache = ref<Map<string, Uint8Array>>(new Map())
const vaultSaltCache = ref<Map<string, Uint8Array>>(new Map())
const vaultDataMap = ref<Map<string, VaultData>>(new Map())
const vaultRoundsMap = ref<Map<string, number>>(new Map())

const unlockDialogVisible = ref(false)
const unlockPassword = ref('')
const unlockingVaultId = ref('')
const isUnlocking = ref(false)

const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  isRoot: false,
  isFolder: false,
  vaultId: '',
  folderId: '',
})

const createVaultVisible = ref(false)
const importVaultVisible = ref(false)
const editVaultVisible = ref(false)
const folderDialogVisible = ref(false)
const passwordDialogVisible = ref(false)
const isEditPassword = ref(false)
const editingPasswordId = ref('')
const isCreatingVault = ref(false)
const isImportingVault = ref(false)
const isSavingVault = ref(false)
const isSavingPassword = ref(false)
const isEditingFolder = ref(false)
const folderFormName = ref('')
const folderParentId = ref<string | null>(null)
const editingFolderId = ref('')
const folderVaultId = ref('')

const generatorConfig = reactive({
  length: 12,
  numbers: true,
  lowercase: true,
  uppercase: true,
  special: false,
})

const createVaultFormRef = ref<FormInstance>()
const importVaultFormRef = ref<FormInstance>()
const passwordFormRef = ref<FormInstance>()

const importVaultForm = reactive({
  name: '',
  description: '',
  password: '',
  path: '',
})

const createVaultForm = reactive({
  name: '',
  description: '',
  password: '',
  rounds: 100,
  path: '',
  filename: '',
})

const editVaultForm = reactive({
  id: '',
  name: '',
  description: '',
  password: '',
  rounds: 100,
  path: '',
  filename: '',
  unlocked: false,
})

const passwordForm = reactive({
  title: '',
  username: '',
  password: '',
  folder_id: null as string | null,
  remark: '',
})

const importVaultRules = computed<FormRules>(() => ({
  name: [{ required: true, message: t('passwords.vaultNameRequired'), trigger: 'blur' }],
  password: [{ required: true, message: t('passwords.masterPasswordRequired'), trigger: 'blur' }],
  path: [{ required: true, message: t('passwords.vaultPathRequired'), trigger: 'change' }],
}))

const createVaultRules = computed<FormRules>(() => ({
  name: [{ required: true, message: t('passwords.vaultNameRequired'), trigger: 'blur' }],
  password: [{ required: true, message: t('passwords.masterPasswordRequired'), trigger: 'blur' }],
  path: [{ required: true, message: t('passwords.vaultPathRequired'), trigger: 'change' }],
  filename: [{ required: true, message: t('passwords.vaultFilenameRequired'), trigger: 'blur' }],
}))

const passwordRules = computed<FormRules>(() => ({
  title: [{ required: true, message: t('passwords.titleRequired'), trigger: 'blur' }],
  username: [{ required: true, message: t('passwords.usernameRequired'), trigger: 'blur' }],
  password: [{ required: true, message: t('passwords.passwordRequired'), trigger: 'blur' }],
}))

const isTestingCreateRounds = ref(false)
const testCreateRoundsResult = ref<number | null>(null)
const isTestingEditRounds = ref(false)
const testEditRoundsResult = ref<number | null>(null)

const VERIFY_STRING = 'BROOKFILE_PASSWORD_VERIFY'

function randomIV(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(16))
}

function randomSalt(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(16))
}

async function buildVaultFileWithKey(key: Uint8Array, salt: Uint8Array, rounds: number, vaultData: VaultData): Promise<string> {
  const encKey = await backend.hmacSha256(key, new TextEncoder().encode('VAULT_ENC_KEY'))
  const macKey = await backend.hmacSha256(key, new TextEncoder().encode('VAULT_MAC_KEY'))
  const sigIv = randomIV()
  const dataIv = randomIV()
  const sigEncrypted = await backend.encrypt(encKey, sigIv, new TextEncoder().encode(VERIFY_STRING))
  const dataEncrypted = await backend.encrypt(encKey, dataIv, new TextEncoder().encode(JSON.stringify(vaultData)))
  const dataEncryptedBytes = new Uint8Array(dataEncrypted)
  const dataForMac = new Uint8Array(dataIv.length + dataEncryptedBytes.length)
  dataForMac.set(dataIv, 0)
  dataForMac.set(dataEncryptedBytes, dataIv.length)
  const dataMac = await backend.hmacSha256(macKey, dataForMac)
  const fileContent = {
    version: 1,
    salt: arrayBufferToBase64(salt.buffer as ArrayBuffer),
    rounds,
    signature_iv: arrayBufferToBase64(sigIv.buffer as ArrayBuffer),
    data_iv: arrayBufferToBase64(dataIv.buffer as ArrayBuffer),
    signature: arrayBufferToBase64(sigEncrypted),
    data: arrayBufferToBase64(dataEncrypted),
    data_mac: arrayBufferToBase64(dataMac.buffer as ArrayBuffer),
  }
  return btoa(JSON.stringify(fileContent))
}

async function openVaultFile(fileBase64: string, masterPassword: string): Promise<{ vaultData: VaultData; key: Uint8Array; salt: Uint8Array; rounds: number }> {
  const fileContent = JSON.parse(atob(fileBase64))
  const { rounds, salt: saltB64, signature_iv, data_iv, signature, data, data_mac } = fileContent
  const salt = new Uint8Array(base64ToArrayBuffer(saltB64))
  const { key } = await backend.deriveKeyAndIV(masterPassword, salt, rounds)
  const encKey = await backend.hmacSha256(key, new TextEncoder().encode('VAULT_ENC_KEY'))
  const macKey = await backend.hmacSha256(key, new TextEncoder().encode('VAULT_MAC_KEY'))
  try {
    const sigIv = new Uint8Array(base64ToArrayBuffer(signature_iv))
    const decrypted = await backend.decrypt(encKey, sigIv, new Uint8Array(base64ToArrayBuffer(signature)))
    const text = new TextDecoder().decode(decrypted)
    if (text !== VERIFY_STRING) throw new Error('WRONG_PASSWORD')
  } catch {
    throw new Error('WRONG_PASSWORD')
  }
  const dataIv = new Uint8Array(base64ToArrayBuffer(data_iv))
  const dataBytes = new Uint8Array(base64ToArrayBuffer(data))
  const dataForMac = new Uint8Array(dataIv.length + dataBytes.length)
  dataForMac.set(dataIv, 0)
  dataForMac.set(dataBytes, dataIv.length)
  const expectedMac = await backend.hmacSha256(macKey, dataForMac)
  const actualMac = new Uint8Array(base64ToArrayBuffer(data_mac))
  if (expectedMac.length !== actualMac.length) throw new Error('INTEGRITY_ERROR')
  let diff = 0
  for (let i = 0; i < expectedMac.length; i++) {
    diff |= expectedMac[i]! ^ actualMac[i]!
  }
  if (diff !== 0) throw new Error('INTEGRITY_ERROR')
  const decryptedData = await backend.decrypt(encKey, dataIv, dataBytes)
  return { vaultData: JSON.parse(new TextDecoder().decode(decryptedData)), key, salt, rounds }
}

async function testEncryptRounds(type: 'create' | 'edit') {
  const rounds = type === 'create' ? createVaultForm.rounds : editVaultForm.rounds
  const roundsRef = type === 'create' ? isTestingCreateRounds : isTestingEditRounds
  const resultRef = type === 'create' ? testCreateRoundsResult : testEditRoundsResult
  roundsRef.value = true
  await new Promise(resolve => setTimeout(resolve, 0))
  const salt = randomSalt()
  const startTime = performance.now()
  await backend.deriveKeyAndIV('password', salt, rounds)
  const elapsed = Math.round(performance.now() - startTime)
  resultRef.value = elapsed
  roundsRef.value = false
}

const vaults = ref<VaultItem[]>([])

async function loadVaults() {
  try {
    const resp = await listVaults()
    vaults.value = (resp.vaults || []).map((v: VaultListItem) => ({
      id: v.id,
      name: v.name,
      description: v.description,
      path: v.path,
      filename: v.filename,
    }))
  } catch {
    vaults.value = []
  }
}

const treeData = computed<TreeNodeData[]>(() => [
  {
    id: 'root',
    label: t('home.categories.passwords'),
    isRoot: true,
    children: vaults.value.map(v => ({
      id: v.id,
      label: v.name,
      isVault: true,
      children: unlockedVaultIds.value.has(v.id) ? buildFolderTree(v.id, null) : undefined,
    })),
  },
])

function buildFolderTree(vaultId: string, parentId: string | null): TreeNodeData[] {
  const data = vaultDataMap.value.get(vaultId)
  if (!data) return []
  return data.folders
    .filter(f => f.parent_id === parentId)
    .map(f => ({
      id: `folder-${f.id}`,
      label: f.name,
      isFolder: true,
      vaultId,
      children: buildFolderTree(vaultId, f.id),
    }))
}

const currentTreeNodeKey = computed(() => {
  if (currentFolderId.value) return `folder-${currentFolderId.value}`
  return currentVault.value?.id || ''
})

const treeProps = {
  children: 'children' as const,
  label: 'label' as const,
}

const currentPasswords = computed(() => {
  if (!currentVault.value) return []
  const data = vaultDataMap.value.get(currentVault.value.id)
  if (!data) return []
  if (currentFolderId.value === null) return data.passwords
  const folderIds = getDescendantFolderIds(currentVault.value.id, currentFolderId.value)
  return data.passwords.filter(p => folderIds.has(p.folder_id || ''))
})

function getDescendantFolderIds(vaultId: string, folderId: string): Set<string> {
  const data = vaultDataMap.value.get(vaultId)
  if (!data) return new Set()
  const ids = new Set<string>()
  ids.add(folderId)
  const folders = data.folders
  function collect(parentId: string) {
    folders.filter(f => f.parent_id === parentId).forEach(f => {
      ids.add(f.id)
      collect(f.id)
    })
  }
  collect(folderId)
  return ids
}

const filteredPasswords = computed(() => {
  const keyword = searchKeyword.value.toLowerCase().trim()
  if (!keyword) return currentPasswords.value
  return currentPasswords.value.filter(
    p => p.title.toLowerCase().includes(keyword) ||
         p.username.toLowerCase().includes(keyword) ||
         p.remark.toLowerCase().includes(keyword)
  )
})

const passwordFolderOptions = computed(() => {
  if (!currentVault.value) return []
  const data = vaultDataMap.value.get(currentVault.value.id)
  if (!data) return []
  const options: Array<{ id: string; label: string; indent: number }> = []
  const folders = data.folders
  function collect(parentId: string | null, indent: number) {
    folders.filter(f => f.parent_id === parentId).forEach(f => {
      options.push({ id: f.id, label: f.name, indent })
      collect(f.id, indent + 1)
    })
  }
  collect(null, 0)
  return options
})

function checkLayout() {
  isMobile.value = window.innerWidth < 768
}

async function selectVault(vault: VaultItem) {
  if (!unlockedVaultIds.value.has(vault.id)) {
    unlockingVaultId.value = vault.id
    unlockPassword.value = ''
    unlockDialogVisible.value = true
    return
  }
  currentVault.value = vault
  currentFolderId.value = null
  searchKeyword.value = ''
}

function handleNodeClick(data: TreeNodeData) {
  if (data.isRoot) {
    currentVault.value = null
    currentFolderId.value = null
    return
  }
  if (data.isFolder) {
    if (!currentVault.value || currentVault.value.id !== data.vaultId) {
      const vault = vaults.value.find(v => v.id === data.vaultId)
      if (vault) {
        currentVault.value = vault
      }
    }
    currentFolderId.value = data.id.replace('folder-', '')
    searchKeyword.value = ''
    return
  }
  const vault = vaults.value.find(v => v.id === data.id)
  if (vault) selectVault(vault)
}

function handleNodeContextMenu(event: MouseEvent, data: TreeNodeData) {
  event.preventDefault()
  contextMenu.x = event.clientX
  contextMenu.y = event.clientY
  contextMenu.isRoot = !!data.isRoot
  contextMenu.isFolder = !!data.isFolder
  contextMenu.vaultId = data.isRoot || data.isFolder ? (data.vaultId || '') : data.id
  contextMenu.folderId = data.isFolder ? data.id.replace('folder-', '') : ''
  contextMenu.visible = true
}

function hideContextMenu() {
  contextMenu.visible = false
}

function handleCreateVault() {
  hideContextMenu()
  createVaultForm.name = ''
  createVaultForm.description = ''
  createVaultForm.password = ''
  createVaultForm.rounds = Math.floor(Math.random() * 40001) + 80000
  createVaultForm.path = ''
  createVaultForm.filename = ''
  testCreateRoundsResult.value = null
  createVaultVisible.value = true
}

function handleImportVault() {
  hideContextMenu()
  importVaultForm.name = ''
  importVaultForm.description = ''
  importVaultForm.password = ''
  importVaultForm.path = ''
  importVaultVisible.value = true
}

function handleEditVault() {
  hideContextMenu()
  const vault = vaults.value.find(v => v.id === contextMenu.vaultId)
  if (!vault) return
  editVaultForm.id = vault.id
  editVaultForm.name = vault.name
  editVaultForm.description = vault.description
  editVaultForm.password = ''
  editVaultForm.rounds = vaultRoundsMap.value.get(vault.id) ?? 100000
  editVaultForm.path = vault.path || '/'
  editVaultForm.filename = vault.filename
  editVaultForm.unlocked = unlockedVaultIds.value.has(vault.id)
  testEditRoundsResult.value = null
  editVaultVisible.value = true
}

async function handleDeleteVault() {
  hideContextMenu()
  const vault = vaults.value.find(v => v.id === contextMenu.vaultId)
  if (!vault) return
  try {
    await ElMessageBox.confirm(
      t('passwords.deleteVaultConfirm', { name: vault.name }),
      t('common.confirm'),
      { type: 'warning' }
    )
  } catch {
    return
  }
  try {
    await deleteVaultApi(vault.id)
    vaults.value = vaults.value.filter(v => v.id !== vault.id)
    unlockedVaultIds.value.delete(vault.id)
    vaultMasterPasswords.value.delete(vault.id)
    vaultKeyCache.value.delete(vault.id)
    vaultSaltCache.value.delete(vault.id)
    vaultDataMap.value.delete(vault.id)
    vaultRoundsMap.value.delete(vault.id)
    if (currentVault.value?.id === vault.id) {
      currentVault.value = null
      currentFolderId.value = null
    }
    ElMessage.success({ __key: 'passwords.deleteVaultSuccess' })
  } catch {
    // error handled by request
  }
}

function handleLockVault() {
  hideContextMenu()
  const vaultId = contextMenu.vaultId
  if (!unlockedVaultIds.value.has(vaultId)) return
  unlockedVaultIds.value.delete(vaultId)
  vaultMasterPasswords.value.delete(vaultId)
  vaultKeyCache.value.delete(vaultId)
  vaultSaltCache.value.delete(vaultId)
  vaultDataMap.value.delete(vaultId)
  vaultRoundsMap.value.delete(vaultId)
  if (currentVault.value?.id === vaultId) {
    currentVault.value = null
    currentFolderId.value = null
  }
  passwordForm.title = ''
  passwordForm.username = ''
  passwordForm.password = ''
  passwordForm.folder_id = null
  passwordForm.remark = ''
  passwordDialogVisible.value = false
  ElMessage.success({ __key: 'passwords.lockVaultSuccess' })
}

function handleCreateFolder() {
  hideContextMenu()
  isEditingFolder.value = false
  folderFormName.value = ''
  if (contextMenu.isFolder) {
    folderParentId.value = contextMenu.folderId
    folderVaultId.value = contextMenu.vaultId
  } else {
    folderParentId.value = null
    folderVaultId.value = contextMenu.vaultId
  }
  folderDialogVisible.value = true
}

function handleRenameFolder() {
  hideContextMenu()
  isEditingFolder.value = true
  folderVaultId.value = contextMenu.vaultId
  editingFolderId.value = contextMenu.folderId
  const data = vaultDataMap.value.get(contextMenu.vaultId)
  const folder = data?.folders.find(f => f.id === contextMenu.folderId)
  folderFormName.value = folder?.name || ''
  folderDialogVisible.value = true
}

async function handleDeleteFolder() {
  hideContextMenu()
  const vaultId = contextMenu.vaultId
  const folderId = contextMenu.folderId
  const data = vaultDataMap.value.get(vaultId)
  if (!data) return
  const folder = data.folders.find(f => f.id === folderId)
  if (!folder) return
  try {
    await ElMessageBox.confirm(
      t('passwords.deleteFolderConfirm', { name: folder.name }),
      t('common.confirm'),
      { type: 'warning' }
    )
  } catch {
    return
  }

  const parentId = folder.parent_id
  for (const p of data.passwords) {
    if (p.folder_id === folderId) {
      p.folder_id = parentId
    }
  }
  for (const f of data.folders) {
    if (f.parent_id === folderId) {
      f.parent_id = parentId
    }
  }
  data.folders = data.folders.filter(f => f.id !== folderId)

  if (currentFolderId.value === folderId) {
    currentFolderId.value = null
  }

  const saved = await saveVaultData(vaultId)
  if (saved) ElMessage.success({ __key: 'passwords.deleteFolderSuccess' })
}

async function submitFolderForm() {
  if (!folderFormName.value.trim()) {
    ElMessage.warning({ __key: 'passwords.folderNameRequired' })
    return
  }
  const vaultId = folderVaultId.value
  const data = vaultDataMap.value.get(vaultId)
  if (!data) return

  if (isEditingFolder.value) {
    const folder = data.folders.find(f => f.id === editingFolderId.value)
    if (folder) {
      folder.name = folderFormName.value.trim()
    }
  } else {
    data.folders.push({
      id: generateId(),
      name: folderFormName.value.trim(),
      parent_id: folderParentId.value,
    })
  }

  const saved = await saveVaultData(vaultId)
  if (saved) {
    folderDialogVisible.value = false
    ElMessage.success({ __key: isEditingFolder.value ? 'passwords.renameFolderSuccess' : 'passwords.createFolderSuccess' })
  }
}

function handleAddPassword() {
  isEditPassword.value = false
  editingPasswordId.value = ''
  passwordForm.title = ''
  passwordForm.username = ''
  passwordForm.password = ''
  passwordForm.folder_id = currentFolderId.value
  passwordForm.remark = ''
  passwordDialogVisible.value = true
}

function handleEditPassword(item: PasswordItem) {
  isEditPassword.value = true
  editingPasswordId.value = item.id
  passwordForm.title = item.title
  passwordForm.username = item.username
  passwordForm.password = item.password
  passwordForm.folder_id = item.folder_id
  passwordForm.remark = item.remark
  passwordDialogVisible.value = true
}

async function handleDeletePassword(item: PasswordItem) {
  try {
    await ElMessageBox.confirm(
      t('passwords.deletePasswordConfirm', { title: item.title }),
      t('common.confirm'),
      { type: 'warning' }
    )
  } catch {
    return
  }
  if (!currentVault.value) return
  const data = vaultDataMap.value.get(currentVault.value.id)
  if (!data) return
  data.passwords = data.passwords.filter(p => p.id !== item.id)
  const saved = await saveVaultData(currentVault.value.id)
  if (saved) ElMessage.success({ __key: 'passwords.deletePasswordSuccess' })
}

function copyText(text: string) {
  navigator.clipboard.writeText(text).then(() => {
    ElMessage.success({ __key: 'passwords.copySuccess' })
  }).catch(() => {
    ElMessage.error({ __key: 'common.error' })
  })
}

function generatePassword() {
  const { length, numbers, lowercase, uppercase, special } = generatorConfig
  if (!numbers && !lowercase && !uppercase && !special) {
    ElMessage.warning({ __key: 'passwords.generatorAtLeastOneType' })
    return
  }
  let chars = ''
  if (numbers) chars += '0123456789'
  if (lowercase) chars += 'abcdefghijklmnopqrstuvwxyz'
  if (uppercase) chars += 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
  if (special) chars += '!@#$%^&*()_+-=[]{}|;:,.<>?'
  const maxValid = Math.floor(4294967296 / chars.length) * chars.length
  let result = ''
  while (result.length < length) {
    const arr = new Uint32Array(length)
    crypto.getRandomValues(arr)
    for (let i = 0; i < arr.length && result.length < length; i++) {
      const val = arr[i]!
      if (val < maxValid) {
        result += chars.charAt(val % chars.length)
      }
    }
  }
  passwordForm.password = result
}

async function downloadFileAsBase64(path: string): Promise<string> {
  const resp = await fetchDownloadFile(path)
  if (!resp.ok) {
    const contentType = resp.headers.get('content-type') || ''
    if (contentType.includes('application/json')) {
      const json = await resp.json()
      if (json.fail_code === 'PATH_NOT_FOUND' || json.fail_code === 'NOT_A_FILE') {
        throw new Error('NOT_FOUND')
      }
      if (json.fail_code === 'NOT_LOGGED_IN') {
        throw new Error('NOT_LOGGED_IN')
      }
    }
    throw new Error('DOWNLOAD_FAILED')
  }
  const buffer = await resp.arrayBuffer()
  const bytes = new Uint8Array(buffer)
  let binary = ''
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i] as number)
  }
  return btoa(binary)
}

async function submitCreateVault() {
  if (!createVaultFormRef.value) return
  const valid = await createVaultFormRef.value.validate().catch(() => false)
  if (!valid) return

  isCreatingVault.value = true
  try {
    const rounds = createVaultForm.rounds
    const salt = randomSalt()
    const { key } = await backend.deriveKeyAndIV(createVaultForm.password, salt, rounds)
    const emptyData: VaultData = { folders: [], passwords: [] }
    const fileDataBase64 = await buildVaultFileWithKey(key, salt, rounds, emptyData)

    const resp = await createVaultApi({
      name: createVaultForm.name,
      description: createVaultForm.description,
      path: createVaultForm.path === '/' ? '' : createVaultForm.path,
      filename: createVaultForm.filename,
      file_data: fileDataBase64,
    })

    if (!resp.success) {
      if (resp.fail_code) {
        ElMessage.error({ __key: `errors.${resp.fail_code}` })
      }
      return
    }

    if (resp.id) {
      vaultMasterPasswords.value.set(resp.id, encodePassword(createVaultForm.password))
      vaultKeyCache.value.set(resp.id, key)
      vaultSaltCache.value.set(resp.id, salt)
      vaultDataMap.value.set(resp.id, emptyData)
      vaultRoundsMap.value.set(resp.id, rounds)
      unlockedVaultIds.value.add(resp.id)
    }

    await loadVaults()
    createVaultVisible.value = false
    ElMessage.success({ __key: 'passwords.createVaultSuccess' })
  } catch {
    ElMessage.error({ __key: 'passwords.createVaultFailed' })
  } finally {
    isCreatingVault.value = false
  }
}

async function submitImportVault() {
  if (!importVaultFormRef.value) return
  const valid = await importVaultFormRef.value.validate().catch(() => false)
  if (!valid) return

  isImportingVault.value = true
  try {
    const filePath = importVaultForm.path
    let fileBase64: string
    try {
      fileBase64 = await downloadFileAsBase64(filePath)
    } catch {
      ElMessage.error({ __key: 'passwords.vaultLoadFailed' })
      return
    }

    let openResult: { vaultData: VaultData; key: Uint8Array; salt: Uint8Array; rounds: number }
    try {
      openResult = await openVaultFile(fileBase64, importVaultForm.password)
    } catch (e: any) {
      if (e?.message === 'INTEGRITY_ERROR') {
        ElMessage.error({ __key: 'passwords.integrityError' })
      } else {
        ElMessage.error({ __key: 'passwords.wrongPassword' })
      }
      return
    }

    const resp = await importVaultApi({
      name: importVaultForm.name,
      description: importVaultForm.description,
      file_path: filePath,
    })

    if (!resp.success) {
      if (resp.fail_code) {
        ElMessage.error({ __key: `errors.${resp.fail_code}` })
      }
      return
    }

    if (resp.id) {
      vaultMasterPasswords.value.set(resp.id, encodePassword(importVaultForm.password))
      vaultKeyCache.value.set(resp.id, openResult.key)
      vaultSaltCache.value.set(resp.id, openResult.salt)
      vaultDataMap.value.set(resp.id, openResult.vaultData)
      vaultRoundsMap.value.set(resp.id, openResult.rounds)
      unlockedVaultIds.value.add(resp.id)
    }

    await loadVaults()
    importVaultVisible.value = false
    ElMessage.success({ __key: 'passwords.importVaultSuccess' })
  } catch {
    // error handled by request
  } finally {
    isImportingVault.value = false
  }
}

async function submitUnlock() {
  const vaultId = unlockingVaultId.value
  if (!vaultId) return

  const vault = vaults.value.find(v => v.id === vaultId)
  if (!vault) return

  const password = unlockPassword.value
  if (!password) {
    ElMessage.warning({ __key: 'passwords.masterPasswordRequired' })
    return
  }

  isUnlocking.value = true
  try {
    const filePath = vault.path ? `${vault.path}/${vault.filename}` : vault.filename
    let fileBase64: string
    try {
      fileBase64 = await downloadFileAsBase64(filePath)
    } catch {
      return
    }

    let openResult: { vaultData: VaultData; key: Uint8Array; salt: Uint8Array; rounds: number }
    try {
      openResult = await openVaultFile(fileBase64, password)
    } catch (e: any) {
      if (e?.message === 'INTEGRITY_ERROR') {
        ElMessage.error({ __key: 'passwords.integrityError' })
      } else {
        ElMessage.error({ __key: 'passwords.wrongPassword' })
      }
      return
    }

    vaultMasterPasswords.value.set(vaultId, encodePassword(password))
    vaultKeyCache.value.set(vaultId, openResult.key)
    vaultSaltCache.value.set(vaultId, openResult.salt)
    vaultDataMap.value.set(vaultId, openResult.vaultData)
    vaultRoundsMap.value.set(vaultId, openResult.rounds)
    unlockedVaultIds.value.add(vaultId)
    unlockDialogVisible.value = false
    currentVault.value = vault
    searchKeyword.value = ''
    ElMessage.success({ __key: 'passwords.openVaultSuccess' })
  } catch {
    ElMessage.error({ __key: 'passwords.vaultLoadFailed' })
  } finally {
    isUnlocking.value = false
  }
}

async function submitEditVault() {
  const vaultId = editVaultForm.id
  if (!vaultId) return

  const changedPassword = editVaultForm.password.length > 0
  const changedRounds = editVaultForm.rounds !== (vaultRoundsMap.value.get(vaultId) ?? 100000)

  if ((changedPassword || changedRounds) && !unlockedVaultIds.value.has(vaultId)) {
    ElMessage.warning({ __key: 'passwords.unlockRequired' })
    return
  }

  const masterPwd = decodePassword(vaultMasterPasswords.value.get(vaultId) || '')
  const activePassword = changedPassword ? editVaultForm.password : masterPwd

  isSavingVault.value = true
  try {
    if (changedPassword) {
      const vaultData = vaultDataMap.value.get(vaultId)
      if (!vaultData || !activePassword) {
        ElMessage.error({ __key: 'passwords.vaultSaveFailed' })
        return
      }
      const newSalt = randomSalt()
      const { key: newKey } = await backend.deriveKeyAndIV(activePassword, newSalt, editVaultForm.rounds)
      const fileDataBase64 = await buildVaultFileWithKey(newKey, newSalt, editVaultForm.rounds, vaultData)
      const resp = await updateVaultApi({
        id: vaultId,
        file_data: fileDataBase64,
      })
      if (!resp.success) return
      const metaResp = await updateVaultMetaApi({
        id: vaultId,
        name: editVaultForm.name,
        description: editVaultForm.description,
      })
      if (!metaResp.success) return
      vaultKeyCache.value.set(vaultId, newKey)
      vaultSaltCache.value.set(vaultId, newSalt)
      vaultMasterPasswords.value.set(vaultId, encodePassword(activePassword))
      vaultRoundsMap.value.set(vaultId, editVaultForm.rounds)
    } else if (changedRounds) {
      const vaultData = vaultDataMap.value.get(vaultId)
      if (!vaultData || !activePassword) {
        ElMessage.error({ __key: 'passwords.vaultSaveFailed' })
        return
      }
      const newSalt = randomSalt()
      const { key: newKey } = await backend.deriveKeyAndIV(activePassword, newSalt, editVaultForm.rounds)
      const fileDataBase64 = await buildVaultFileWithKey(newKey, newSalt, editVaultForm.rounds, vaultData)
      const resp = await updateVaultApi({
        id: vaultId,
        file_data: fileDataBase64,
      })
      if (!resp.success) return
      const metaResp = await updateVaultMetaApi({
        id: vaultId,
        name: editVaultForm.name,
        description: editVaultForm.description,
      })
      if (!metaResp.success) return
      vaultKeyCache.value.set(vaultId, newKey)
      vaultSaltCache.value.set(vaultId, newSalt)
      vaultRoundsMap.value.set(vaultId, editVaultForm.rounds)
    } else {
      const resp = await updateVaultMetaApi({
        id: vaultId,
        name: editVaultForm.name,
        description: editVaultForm.description,
      })
      if (!resp.success) return
    }

    const vault = vaults.value.find(v => v.id === vaultId)
    if (vault) {
      vault.name = editVaultForm.name
      vault.description = editVaultForm.description
      if (currentVault.value?.id === vaultId) {
        currentVault.value = { ...vault, name: editVaultForm.name, description: editVaultForm.description }
      }
    }
    editVaultVisible.value = false
    ElMessage.success({ __key: 'passwords.editVaultSuccess' })
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'passwords.vaultSaveFailed' })
    }
  } finally {
    isSavingVault.value = false
  }
}

async function saveVaultData(vaultId: string): Promise<boolean> {
  if (isSavingVault.value) return false
  isSavingVault.value = true
  try {
    const vault = vaults.value.find(v => v.id === vaultId)
    const key = vaultKeyCache.value.get(vaultId)
    const salt = vaultSaltCache.value.get(vaultId)
    const vaultData = vaultDataMap.value.get(vaultId)
    const rounds = vaultRoundsMap.value.get(vaultId) ?? 100000

    if (!vault || !key || !salt || !vaultData) {
      ElMessage.error({ __key: 'passwords.vaultSaveFailed' })
      return false
    }

    try {
      const fileDataBase64 = await buildVaultFileWithKey(key, salt, rounds, vaultData)
      const filePath = vault.path ? `${vault.path}/${vault.filename}` : vault.filename
      await uploadSingleFile(filePath, fileDataBase64)
      return true
    } catch (error: any) {
      if (error.isAxiosError && !error.response) {
        ElMessage.error({ __key: 'passwords.vaultSaveRetry' })
      }
      return false
    }
  } finally {
    isSavingVault.value = false
  }
}

async function submitPasswordForm() {
  if (!passwordFormRef.value || !currentVault.value) return
  const valid = await passwordFormRef.value.validate().catch(() => false)
  if (!valid) return

  const vaultId = currentVault.value.id
  const data = vaultDataMap.value.get(vaultId)
  if (!data) return

  if (isEditPassword.value) {
    const item = data.passwords.find(p => p.id === editingPasswordId.value)
    if (item) {
      const backup = { ...item }
      item.title = passwordForm.title
      item.username = passwordForm.username
      item.password = passwordForm.password
      item.folder_id = passwordForm.folder_id
      item.remark = passwordForm.remark
      item.updated_at = new Date().toISOString()

      isSavingPassword.value = true
      const saved = await saveVaultData(vaultId)
      isSavingPassword.value = false

      if (saved) {
        passwordDialogVisible.value = false
        ElMessage.success({ __key: 'passwords.editPasswordSuccess' })
      } else {
        Object.assign(item, backup)
      }
    }
  } else {
    const newItem: PasswordItem = {
      id: generateId(),
      folder_id: passwordForm.folder_id,
      title: passwordForm.title,
      username: passwordForm.username,
      password: passwordForm.password,
      remark: passwordForm.remark,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }
    data.passwords.push(newItem)

    isSavingPassword.value = true
    const saved = await saveVaultData(vaultId)
    isSavingPassword.value = false

    if (saved) {
      passwordDialogVisible.value = false
      ElMessage.success({ __key: 'passwords.addPasswordSuccess' })
    } else {
      const idx = data.passwords.indexOf(newItem)
      if (idx !== -1) data.passwords.splice(idx, 1)
    }
  }
}

onMounted(() => {
  checkLayout()
  window.addEventListener('resize', checkLayout)
  loadVaults()
})

onUnmounted(() => {
  window.removeEventListener('resize', checkLayout)
})
</script>

<style scoped>
.passwords-container {
  height: 100%;
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
}

.vault-panel {
  width: 240px;
  flex-shrink: 0;
  border-right: 1px solid var(--el-border-color-lighter);
  overflow-y: auto;
  padding: 12px 0;
}

.passwords-container:not(:has(.mobile-vault-list)) {
  flex-direction: row;
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
}

.tree-node-locked {
  color: var(--el-text-color-secondary);
}

.tree-node-unlocked {
  color: var(--el-color-primary);
}

.tree-node-folder {
  color: var(--el-color-warning);
}

.password-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.password-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  flex-shrink: 0;
}

.search-input {
  width: 280px;
}

.table-wrapper {
  flex: 1;
  overflow: hidden;
  padding: 0 16px 16px;
}

.btn-text {
  margin-left: 6px;
}

.password-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--el-text-color-secondary);
  gap: 12px;
}

.context-menu {
  position: fixed;
  z-index: 2000;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 4px;
  padding: 4px 0;
  box-shadow: var(--el-box-shadow-light);
}

.context-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 14px;
  color: var(--el-text-color-regular);
  white-space: nowrap;
}

.context-menu-item:hover {
  background: var(--el-fill-color-light);
  color: var(--el-color-primary);
}

.context-menu-danger:hover {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}

.context-menu-divider {
  height: 1px;
  margin: 4px 0;
  background: var(--el-border-color-lighter);
}

.mobile-vault-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.mobile-vault-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
  margin-bottom: 8px;
  cursor: pointer;
}

.mobile-vault-card:active {
  background: var(--el-fill-color);
}

.mobile-vault-icon {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--el-color-primary-light-9);
  border-radius: 8px;
  color: var(--el-color-primary);
  flex-shrink: 0;
}

.mobile-vault-info {
  flex: 1;
  min-width: 0;
}

.mobile-vault-name {
  font-size: 15px;
  font-weight: 500;
  color: var(--el-text-color-primary);
}

.mobile-vault-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mobile-vault-arrow {
  color: var(--el-text-color-placeholder);
  flex-shrink: 0;
}

.mobile-empty {
  text-align: center;
  color: var(--el-text-color-secondary);
  padding: 40px 0;
  font-size: 14px;
}

.mobile-pwd-view {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

.mobile-pwd-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  flex-shrink: 0;
}

.mobile-pwd-back {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  font-size: 15px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.mobile-pwd-back span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mobile-pwd-back:active {
  color: var(--el-text-color-secondary);
}

.mobile-pwd-search {
  margin: 12px 0;
  flex-shrink: 0;
}

.mobile-pwd-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 12px 12px;
}

.mobile-pwd-card {
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
  padding: 14px 16px;
  margin-bottom: 8px;
}

.mobile-pwd-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 10px;
}

.mobile-pwd-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  cursor: pointer;
}

.mobile-pwd-row:active {
  opacity: 0.7;
}

.mobile-pwd-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
  width: 36px;
}

.mobile-pwd-value {
  font-size: 13px;
  color: var(--el-text-color-regular);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mobile-pwd-copy {
  color: var(--el-text-color-placeholder);
  flex-shrink: 0;
  font-size: 14px;
}

.mobile-pwd-remark {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 6px;
  padding-top: 6px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.mobile-pwd-actions {
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.pwd-generator {
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
  padding: 12px 14px;
  margin-bottom: 16px;
}

.generator-header {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 10px;
}

.generator-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.generator-label {
  font-size: 13px;
  color: var(--el-text-color-regular);
}

.generator-checks {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
}

.encrypt-rounds-wrapper {
  width: 100%;
}

.encrypt-rounds-wrapper .el-slider {
  margin: 0 10px 8px;
}

.encrypt-rounds-wrapper .el-button {
  margin-bottom: 6px;
}

.encrypt-rounds-result {
  color: var(--el-color-primary);
  font-size: 13px;
  margin-bottom: 4px;
}

.encrypt-rounds-hint {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
</style>
