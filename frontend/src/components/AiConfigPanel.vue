<template>
  <div class="ai-config">
    <div class="ai-config__header">
      <el-button type="primary" plain @click="openAddDialog">{{ t('ai.addProvider') }}</el-button>
    </div>

    <div v-loading="loading" class="ai-config__body">
      <el-empty v-if="!loading && providers.length === 0" :description="t('ai.noProviders')" />
      <div v-else class="provider-list">
        <div v-for="provider in providers" :key="provider.id" class="provider-card">
          <div class="provider-card__head">
            <div class="provider-card__info">
              <div class="provider-card__name">
                {{ provider.name }}
                <span class="provider-card__type">{{ presetName(provider.provider_type) }}</span>
              </div>
              <div class="provider-card__url">{{ provider.base_url }}</div>
            </div>
            <div class="provider-card__actions">
              <el-link type="primary" @click="openEditProvider(provider)">{{ t('ai.edit') }}</el-link>
              <el-link type="danger" @click="handleDeleteProvider(provider)">{{ t('ai.delete') }}</el-link>
            </div>
          </div>
          <div class="provider-card__models">
            <div class="models-title">{{ t('ai.models') }}</div>
            <div v-if="provider.models.length === 0" class="models-empty">{{ t('ai.noModels') }}</div>
            <div v-for="model in provider.models" :key="model.id" class="model-row">
              <span class="model-id">{{ model.model_id }}</span>
              <span v-if="model.supports_vision" class="model-badge">{{ t('ai.supportsVision') }}</span>
              <span v-if="model.context_length > 0" class="model-meta">{{ t('ai.contextLength') }}: {{ model.context_length }}</span>
              <span v-if="model.max_output_tokens > 0" class="model-meta">{{ t('ai.maxOutput') }}: {{ model.max_output_tokens }}</span>
              <span class="model-actions">
                <el-link type="danger" @click="handleDeleteModel(model)">{{ t('ai.delete') }}</el-link>
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 添加供应商向导 -->
    <el-dialog v-model="addVisible" :title="t('ai.addProvider')" :width="dialogWidth" :close-on-click-modal="false" @closed="resetAddDialog">
      <div class="dialog-body">
        <el-steps :active="addStep - 1" simple style="margin-bottom: 20px">
        <el-step :title="t('ai.providerInfo')" />
        <el-step :title="t('ai.configureModels')" />
      </el-steps>

      <div v-if="addStep === 1" class="wizard-form">
        <el-form label-position="top">
          <el-form-item :label="t('ai.providerType')">
            <el-select v-model="addForm.provider_type" style="width: 100%" @change="addForm.base_url = ''">
              <el-option v-for="p in presets" :key="p.type_id" :label="p.name" :value="p.type_id" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('ai.providerName')">
            <el-input v-model="addForm.name" :placeholder="t('ai.providerNamePh')" />
          </el-form-item>
          <el-form-item :label="t('ai.baseUrl')">
            <el-input v-model="addForm.base_url" :placeholder="addBaseUrlPlaceholder" />
          </el-form-item>
          <el-form-item :label="t('ai.apiKey')">
            <el-input v-model="addForm.api_key" type="password" show-password />
          </el-form-item>
          <el-form-item :label="t('ai.proxy')">
            <el-input v-model="addForm.proxy" :placeholder="t('ai.proxyPh')" />
          </el-form-item>
        </el-form>
      </div>

      <div v-else class="wizard-models">
        <div v-if="addLoadFailed" class="fetch-failed">
          <el-alert :title="t('ai.fetchModelsFailed')" :description="t('ai.fetchModelsFailedTip')" type="warning" :closable="false" />
        </div>
        <div v-else-if="fetchedModels.length > 0" class="fetched-block">
          <div class="block-label">{{ t('ai.availableModels') }}</div>
          <div class="fetched-list">
            <el-checkbox
              v-for="m in fetchedModels"
              :key="m"
              :model-value="draftSelected(drafts, m)"
              :disabled="draftFetchDisabled(drafts, m)"
              @change="(v: boolean) => toggleFetchModel(drafts, m, v)"
              class="fetched-option"
            >{{ m }}</el-checkbox>
          </div>
        </div>

        <div class="add-manual-row">
          <el-button size="small" @click="addManualDraft(drafts)">{{ t('ai.addModel') }}</el-button>
        </div>

        <el-table v-if="drafts.length > 0" :data="drafts" size="small" class="draft-table">
          <el-table-column :label="t('ai.modelId')" min-width="160">
            <template #default="{ row }">
              <el-input v-model="row.model_id" :placeholder="t('ai.modelIdPh')" />
            </template>
          </el-table-column>
          <el-table-column :label="t('ai.supportsVision')" width="100" align="center">
            <template #default="{ row }">
              <el-switch v-model="row.supports_vision" />
            </template>
          </el-table-column>
          <el-table-column :label="t('ai.contextLength')" width="130">
            <template #default="{ row }">
              <el-input-number v-model="row.context_length" :min="0" :controls="false" style="width: 100%" />
            </template>
          </el-table-column>
          <el-table-column :label="t('ai.maxOutput')" width="130">
            <template #default="{ row }">
              <el-input-number v-model="row.max_output_tokens" :min="0" :controls="false" style="width: 100%" />
            </template>
          </el-table-column>
          <el-table-column width="60" align="center">
            <template #default="{ $index }">
              <el-link type="danger" @click="drafts.splice($index, 1)">{{ t('ai.delete') }}</el-link>
            </template>
          </el-table-column>
        </el-table>
      </div>
      </div>

      <template #footer>
        <div v-if="addStep === 1">
          <el-button @click="addVisible = false">{{ t('common.cancel') }}</el-button>
          <el-button type="primary" :loading="nextLoading" @click="handleProviderToModels">{{ t('ai.next') }}</el-button>
        </div>
        <div v-else>
          <el-button @click="addStep = 1">{{ t('common.back') }}</el-button>
          <el-button type="primary" :loading="savingModels" @click="handleSaveModels">{{ t('ai.save') }}</el-button>
        </div>
      </template>
    </el-dialog>

    <!-- 编辑供应商向导 -->
    <el-dialog v-model="editVisible" :title="t('ai.editProvider')" :width="dialogWidth" :close-on-click-modal="false" @closed="resetEditDialog">
      <div class="dialog-body">
        <el-steps :active="editStep - 1" simple style="margin-bottom: 20px">
          <el-step :title="t('ai.providerInfo')" />
          <el-step :title="t('ai.configureModels')" />
        </el-steps>

        <div v-if="editStep === 1" class="wizard-form">
          <el-form label-position="top">
            <el-form-item :label="t('ai.providerType')">
              <el-select v-model="editForm.provider_type" style="width: 100%">
                <el-option v-for="p in presets" :key="p.type_id" :label="p.name" :value="p.type_id" />
              </el-select>
            </el-form-item>
            <el-form-item :label="t('ai.providerName')">
              <el-input v-model="editForm.name" />
            </el-form-item>
            <el-form-item :label="t('ai.baseUrl')">
              <el-input v-model="editForm.base_url" :placeholder="editBaseUrlPlaceholder" />
            </el-form-item>
            <el-form-item :label="t('ai.apiKey')">
              <el-input v-model="editForm.api_key" type="password" show-password :placeholder="t('ai.apiKeyKeepHint')" />
            </el-form-item>
            <el-form-item :label="t('ai.proxy')">
              <el-input v-model="editForm.proxy" :placeholder="t('ai.proxyPh')" />
            </el-form-item>
          </el-form>
        </div>

        <div v-else class="wizard-models">
          <div v-if="editLoadFailed" class="fetch-failed">
            <el-alert :title="t('ai.fetchModelsFailed')" :description="t('ai.fetchModelsFailedTip')" type="warning" :closable="false" />
          </div>
          <div v-else-if="editFetchedModels.length > 0" class="fetched-block">
            <div class="block-label">{{ t('ai.availableModels') }}</div>
            <div class="fetched-list">
              <el-checkbox
                v-for="m in editFetchedModels"
                :key="m"
                :model-value="draftSelected(editDrafts, m)"
                :disabled="draftFetchDisabled(editDrafts, m)"
                @change="(v: boolean) => toggleFetchModel(editDrafts, m, v)"
                class="fetched-option"
              >{{ m }}</el-checkbox>
            </div>
          </div>

          <div class="add-manual-row">
            <el-button size="small" @click="addManualDraft(editDrafts)">{{ t('ai.addModel') }}</el-button>
          </div>

          <el-table v-if="editDrafts.length > 0" :data="editDrafts" size="small" class="draft-table">
            <el-table-column :label="t('ai.modelId')" min-width="160">
              <template #default="{ row }">
                <el-input v-model="row.model_id" :placeholder="t('ai.modelIdPh')" />
              </template>
            </el-table-column>
            <el-table-column :label="t('ai.supportsVision')" width="100" align="center">
              <template #default="{ row }">
                <el-switch v-model="row.supports_vision" />
              </template>
            </el-table-column>
            <el-table-column :label="t('ai.contextLength')" width="130">
              <template #default="{ row }">
                <el-input-number v-model="row.context_length" :min="0" :controls="false" style="width: 100%" />
              </template>
            </el-table-column>
            <el-table-column :label="t('ai.maxOutput')" width="130">
              <template #default="{ row }">
                <el-input-number v-model="row.max_output_tokens" :min="0" :controls="false" style="width: 100%" />
              </template>
            </el-table-column>
            <el-table-column width="60" align="center">
              <template #default="{ $index }">
                <el-link type="danger" @click="editDrafts.splice($index, 1)">{{ t('ai.delete') }}</el-link>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </div>

      <template #footer>
        <div v-if="editStep === 1">
          <el-button @click="editVisible = false">{{ t('common.cancel') }}</el-button>
          <el-button type="primary" :loading="editNextLoading" @click="handleEditToModels">{{ t('ai.next') }}</el-button>
        </div>
        <div v-else>
          <el-button @click="editStep = 1">{{ t('common.back') }}</el-button>
          <el-button type="primary" :loading="editSaving" @click="handleSaveProvider">{{ t('ai.save') }}</el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  listAiProviders,
  listAiProviderPresets,
  createAiProvider,
  updateAiProvider,
  deleteAiProvider,
  fetchAiModels,
  deleteAiModel,
  type AiProviderItem,
  type AiProviderPreset,
  type AiModelItem,
} from '@/api/system'

const { t } = useI18n()

const providers = ref<AiProviderItem[]>([])
const loading = ref(true)
const presets = ref<AiProviderPreset[]>([])

function presetByType(type: string): AiProviderPreset | undefined {
  return presets.value.find((p) => p.type_id === type)
}

function presetName(type: string): string {
  return presetByType(type)?.name || type
}

// 添加向导
const addVisible = ref(false)
const addStep = ref(1)
const addForm = ref({ name: '', provider_type: 'openai_completions', base_url: '', api_key: '', proxy: '' })
const DEFAULT_CONTEXT_LENGTH = 200000
const DEFAULT_MAX_OUTPUT = 65536
const nextLoading = ref(false)
const addLoadFailed = ref(false)
const fetchedModels = ref<string[]>([])
const savingModels = ref(false)

const addBaseUrlPlaceholder = computed(
  () => presetByType(addForm.value.provider_type)?.default_base_url || t('ai.baseUrlPh'),
)
const editBaseUrlPlaceholder = computed(
  () => presetByType(editForm.value.provider_type)?.default_base_url || t('ai.baseUrlPh'),
)

interface Draft {
  key: string
  id?: string
  model_id: string
  supports_vision: boolean
  context_length: number
  max_output_tokens: number
  source: 'existing' | 'fetch' | 'manual'
}
const drafts = ref<Draft[]>([])

// 编辑供应商向导
const editVisible = ref(false)
const editStep = ref(1)
const editForm = ref({ name: '', provider_type: 'openai_completions', base_url: '', api_key: '', proxy: '' })
const editProviderId = ref('')
const editNextLoading = ref(false)
const editLoadFailed = ref(false)
const editFetchedModels = ref<string[]>([])
const editDrafts = ref<Draft[]>([])
const editSaving = ref(false)

function draftSelected(target: Draft[], modelId: string): boolean {
  return target.some((d) => d.source === 'fetch' && d.model_id === modelId)
}

function draftFetchDisabled(target: Draft[], modelId: string): boolean {
  return target.some((d) => d.source !== 'fetch' && d.model_id === modelId)
}

function toggleFetchModel(target: Draft[], modelId: string, checked: boolean) {
  if (checked) {
    if (!target.some((d) => d.model_id === modelId)) {
      target.push({
        key: crypto.randomUUID(),
        model_id: modelId,
        supports_vision: false,
        context_length: DEFAULT_CONTEXT_LENGTH,
        max_output_tokens: DEFAULT_MAX_OUTPUT,
        source: 'fetch',
      })
    }
  } else {
    const idx = target.findIndex((d) => d.source === 'fetch' && d.model_id === modelId)
    if (idx >= 0) target.splice(idx, 1)
  }
}

function addManualDraft(target: Draft[]) {
  target.push({
    key: crypto.randomUUID(),
    model_id: '',
    supports_vision: false,
    context_length: DEFAULT_CONTEXT_LENGTH,
    max_output_tokens: DEFAULT_MAX_OUTPUT,
    source: 'manual',
  })
}

function proxyInvalid(proxy: string): boolean {
  const v = proxy.trim()
  return v !== '' && !/^http:\/\/.+/.test(v)
}

async function load() {
  loading.value = true
  try {
    const res = await listAiProviders()
    providers.value = res.providers
  } catch {} finally {
    loading.value = false
  }
}

async function loadPresets() {
  try {
    const res = await listAiProviderPresets()
    presets.value = res.presets
  } catch {}
}

// 弹窗宽度自适应（移动端 92% / 桌面端固定）
const dialogWidth = ref('640px')
function updateDialogWidths() {
  dialogWidth.value = window.innerWidth < 700 ? '92%' : '640px'
}
onMounted(() => {
  updateDialogWidths()
  window.addEventListener('resize', updateDialogWidths)
})
onUnmounted(() => window.removeEventListener('resize', updateDialogWidths))

onMounted(load)
onMounted(loadPresets)

function openAddDialog() {
  addStep.value = 1
  addForm.value = { name: '', provider_type: 'openai_completions', base_url: '', api_key: '', proxy: '' }
  addVisible.value = true
}

function resetAddDialog() {
  addStep.value = 1
  addLoadFailed.value = false
  fetchedModels.value = []
  drafts.value = []
}

async function handleProviderToModels() {
  if (!addForm.value.name.trim()) {
    ElMessage.warning(t('ai.enterName'))
    return
  }
  const baseUrl = addForm.value.base_url.trim()
  if (baseUrl && !/^https?:\/\/.+/.test(baseUrl)) {
    ElMessage.warning(t('ai.baseUrlInvalid'))
    return
  }
  if (presetByType(addForm.value.provider_type)?.requires_api_key && !addForm.value.api_key.trim()) {
    ElMessage.warning(t('ai.enterApiKey'))
    return
  }
  if (proxyInvalid(addForm.value.proxy)) {
    ElMessage.warning(t('ai.proxyInvalid'))
    return
  }

  nextLoading.value = true
  try {
    addLoadFailed.value = false
    fetchedModels.value = []
    drafts.value = []
    try {
      const fetchRes = await fetchAiModels(
        addForm.value.provider_type,
        baseUrl || undefined,
        addForm.value.api_key.trim() || undefined,
        addForm.value.proxy.trim() || undefined,
      )
      fetchedModels.value = fetchRes.models
    } catch {
      addLoadFailed.value = true
    }
    addStep.value = 2
  } finally {
    nextLoading.value = false
  }
}

async function handleSaveModels() {
  const all = drafts.value.map((d) => d.model_id.trim())
  if (drafts.value.length === 0 || all.some((id) => !id)) {
    ElMessage.warning(t('ai.enterModelId'))
    return
  }
  savingModels.value = true
  try {
    await createAiProvider({
      name: addForm.value.name.trim(),
      provider_type: addForm.value.provider_type,
      base_url: addForm.value.base_url.trim().replace(/\/+$/, '') || undefined,
      api_key: addForm.value.api_key.trim(),
      proxy: addForm.value.proxy.trim() || undefined,
      models: drafts.value.map((d) => ({
        model_id: d.model_id.trim(),
        supports_vision: d.supports_vision,
        context_length: d.context_length,
        max_output_tokens: d.max_output_tokens,
      })),
    })
    ElMessage.success(t('ai.saveSuccess'))
    addVisible.value = false
    await load()
  } catch {} finally {
    savingModels.value = false
  }
}

function openEditProvider(provider: AiProviderItem) {
  editProviderId.value = provider.id
  editForm.value = {
    name: provider.name,
    provider_type: provider.provider_type,
    base_url: provider.base_url,
    api_key: '',
    proxy: provider.proxy,
  }
  editStep.value = 1
  editLoadFailed.value = false
  editFetchedModels.value = []
  editDrafts.value = provider.models.map((m) => ({
    key: m.id,
    id: m.id,
    model_id: m.model_id,
    supports_vision: m.supports_vision,
    context_length: m.context_length,
    max_output_tokens: m.max_output_tokens,
    source: 'existing',
  }))
  editVisible.value = true
}

function resetEditDialog() {
  editStep.value = 1
  editLoadFailed.value = false
  editFetchedModels.value = []
  editDrafts.value = []
}

async function handleEditToModels() {
  if (!editForm.value.name.trim()) {
    ElMessage.warning(t('ai.enterName'))
    return
  }
  const baseUrl = editForm.value.base_url.trim()
  if (baseUrl && !/^https?:\/\/.+/.test(baseUrl)) {
    ElMessage.warning(t('ai.baseUrlInvalid'))
    return
  }
  if (proxyInvalid(editForm.value.proxy)) {
    ElMessage.warning(t('ai.proxyInvalid'))
    return
  }

  editNextLoading.value = true
  try {
    editLoadFailed.value = false
    editFetchedModels.value = []
    try {
      const fetchRes = await fetchAiModels(
        editForm.value.provider_type,
        baseUrl || undefined,
        editForm.value.api_key.trim() || undefined,
        editForm.value.proxy.trim() || undefined,
        editProviderId.value,
      )
      editFetchedModels.value = fetchRes.models
    } catch {
      editLoadFailed.value = true
    }
    editStep.value = 2
  } finally {
    editNextLoading.value = false
  }
}

async function handleSaveProvider() {
  const all = editDrafts.value.map((d) => d.model_id.trim())
  if (all.some((id) => !id)) {
    ElMessage.warning(t('ai.enterModelId'))
    return
  }
  editSaving.value = true
  try {
    await updateAiProvider({
      id: editProviderId.value,
      name: editForm.value.name.trim(),
      provider_type: editForm.value.provider_type,
      base_url: editForm.value.base_url.trim().replace(/\/+$/, '') || undefined,
      api_key: editForm.value.api_key.trim() || undefined,
      proxy: editForm.value.proxy.trim(),
      models: editDrafts.value.map((d) => ({
        id: d.id || undefined,
        model_id: d.model_id.trim(),
        supports_vision: d.supports_vision,
        context_length: d.context_length,
        max_output_tokens: d.max_output_tokens,
      })),
    })
    ElMessage.success(t('ai.saveSuccess'))
    editVisible.value = false
    await load()
  } catch {} finally {
    editSaving.value = false
  }
}

async function handleDeleteProvider(provider: AiProviderItem) {
  try {
    await ElMessageBox.confirm(t('ai.deleteProviderConfirm'), { type: 'warning' })
  } catch { return }
  try {
    await deleteAiProvider(provider.id)
    ElMessage.success(t('ai.saveSuccess'))
    await load()
  } catch {}
}

async function handleDeleteModel(model: AiModelItem) {
  try {
    await ElMessageBox.confirm(t('ai.deleteModelConfirm'), { type: 'warning' })
  } catch { return }
  try {
    await deleteAiModel(model.id)
    ElMessage.success(t('ai.saveSuccess'))
    await load()
  } catch {}
}
</script>

<style scoped>
.ai-config {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
  overflow-x: hidden;
}

.dialog-body {
  max-height: 60vh;
  overflow-y: auto;
}

.ai-config__header {
  display: flex;
  justify-content: flex-end;
}

.ai-config__body {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.provider-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.provider-card {
  border: 1px solid var(--el-border-color-light);
  border-radius: 12px;
  padding: 16px;
}

.provider-card__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.provider-card__name {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.provider-card__type {
  font-size: 12px;
  font-weight: 400;
  padding: 1px 8px;
  border-radius: 10px;
  background: var(--el-fill-color);
  color: var(--el-text-color-secondary);
}

.provider-card__url {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
  word-break: break-all;
}

.provider-card__actions {
  display: flex;
  gap: 12px;
  flex-shrink: 0;
}

.provider-card__models {
  margin-top: 14px;
  border-top: 1px dashed var(--el-border-color-lighter);
  padding-top: 12px;
}

.models-title {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
}

.models-empty {
  font-size: 13px;
  color: var(--el-text-color-placeholder);
}

.model-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 12px;
  padding: 6px 0;
  font-size: 13px;
}

.model-id {
  font-weight: 500;
  color: var(--el-text-color-primary);
  min-width: 120px;
}

.model-badge {
  font-size: 12px;
  padding: 1px 8px;
  border-radius: 10px;
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
  flex-shrink: 0;
}

.model-meta {
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}

.model-actions {
  margin-left: auto;
  display: flex;
  gap: 12px;
  flex-shrink: 0;
}

.fetch-failed {
  margin-bottom: 16px;
}

.fetched-block {
  margin-bottom: 12px;
}

.block-label,
.drafts-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 8px;
}

.fetched-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0 16px;
  max-height: 160px;
  overflow-y: auto;
  padding: 4px;
}

.add-manual-row {
  margin-bottom: 12px;
}

.draft-table {
  width: 100%;
}

.wizard-form :deep(.el-form-item) {
  margin-bottom: 16px;
}
</style>
