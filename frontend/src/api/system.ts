import axios from 'axios'
import { ElMessage } from '@/utils/message'
import router from '@/router'
import { useUserStore } from '@/stores/user'

const api = axios.create({
  baseURL: '/api',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
  withCredentials: true,
})

export interface ApiResponse<T = unknown> {
  success: boolean
  fail_code?: string
  data?: T
}

function handleNotLoggedIn() {
  const userStore = useUserStore()
  userStore.logout()
  ElMessage.warning({ __key: 'accountManagement.sessionExpired' })
  router.push('/login')
}

async function request<T>(config: Parameters<typeof api.request>[0] & { skipErrorMessage?: boolean; rawResponse?: boolean }): Promise<T> {
  try {
    const response = await api.request<T & { success?: boolean; fail_code?: string; message?: string }>(config)
    if (response.data?.fail_code && !config.rawResponse) {
      if (!config.skipErrorMessage) {
        if (response.data.fail_code === 'NOT_LOGGED_IN') {
          handleNotLoggedIn()
        } else {
          ElMessage.error({ __key: `errors.${response.data.fail_code}` })
        }
      }
      const err = new Error(response.data.fail_code)
      ;(err as any).detailMessage = response.data.message
      throw err
    }
    return response.data
  } catch (error: any) {
    if (error.response) {
      const failCode = error.response?.data?.fail_code
      if (failCode === 'NOT_LOGGED_IN') {
        if (!config.skipErrorMessage) {
          handleNotLoggedIn()
        }
      } else if (!config.skipErrorMessage) {
        if (failCode) {
          ElMessage.error({ __key: `errors.${failCode}` })
        } else {
          ElMessage.error({ __key: 'errors.NETWORK_ERROR' })
        }
      }
    }
    throw error
  }
}

async function requestWithSuccess<T extends ApiResponse>(config: Parameters<typeof api.request>[0] & { skipErrorMessage?: boolean }): Promise<T> {
  const response = await request<T>(config)
  if (!response.success) {
    throw new Error(response.fail_code || 'UNKNOWN_ERROR')
  }
  return response
}

export interface SystemInfoResponse {
  initialized: boolean
  logged_in: boolean
  system_name: string
  user?: UserInfo
}

export interface UserInfo {
  id?: string
  username: string
  is_admin?: boolean
  feature_order?: string
  recycle_bin_enabled?: boolean
  has_shares?: boolean
}

export interface InitRequest {
  username: string
  password: string
  system_name: string
  root_path: string
  recycle_bin_path?: string
}

export interface BrowseFolder {
  name: string
  path: string
}

export interface BrowseResponse {
  folders: BrowseFolder[]
  has_parent: boolean
  parent_path: string | null
}

export interface FileItem {
  name: string
  file_type: 'directory' | 'file' | 'other' | 'parent'
  size: number
  modified: string
}

export interface FileBrowseResponse {
  files: FileItem[]
}

export interface LoginRequest {
  username: string
  password: string
}

export async function getSystemInfo(skipErrorMessage?: boolean): Promise<SystemInfoResponse> {
  return request({ method: 'POST', url: '/system/info', skipErrorMessage })
}

export async function initSystem(data: InitRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/system/init', data })
}

export async function login(data: LoginRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/auth/login', data })
}

export async function ping(): Promise<void> {
  try {
    await request<ApiResponse>({ method: 'POST', url: '/auth/ping', skipErrorMessage: true })
  } catch {
  }
}

export async function logout(): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/auth/logout' })
}

export interface UserItem {
  id: string
  username: string
  root_path: string | null
  recycle_bin_path: string | null
  is_admin: boolean
  expire_at: string | null
  remark: string | null
  feature_order: string
  created_at: string | null
  updated_at: string | null
}

export interface CreateUserRequest {
  username: string
  password: string
  root_path?: string
  is_admin?: boolean
  expire_at?: string
  remark?: string
  recycle_bin_path?: string
}

export interface UpdateUserRequest {
  id: string
  password?: string
  root_path?: string
  is_admin?: boolean
  expire_at?: string
  remark?: string
  recycle_bin_path?: string | null
}

export interface DeleteUserRequest {
  id: string
}

export async function getUserList(): Promise<UserItem[]> {
  return request({ method: 'POST', url: '/user/list' })
}

export async function getUser(id: string): Promise<UserItem> {
  return request({ method: 'POST', url: '/user/get', data: { id } })
}

export async function createUser(data: CreateUserRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/create', data })
}

export async function updateUser(data: UpdateUserRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/update', data })
}

export async function deleteUser(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/delete', data: { id } })
}

export async function uploadAvatar(file: File): Promise<ApiResponse> {
  const formData = new FormData()
  formData.append('avatar', file)
  return requestWithSuccess({
    method: 'POST',
    url: '/user/upload_avatar',
    data: formData,
    headers: { 'Content-Type': 'multipart/form-data' },
  })
}

export async function fetchAvatar(id: string): Promise<Blob | null> {
  try {
    const response = await fetch('/api/user/get_avatar', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ id }),
      credentials: 'include',
    })
    const contentType = response.headers.get('Content-Type')
    if (contentType && contentType.includes('application/json')) {
      const data = await response.json()
      if (data.fail_code === 'NOT_LOGGED_IN') {
        handleNotLoggedIn()
      }
      return null
    }
    if (contentType && contentType.startsWith('image/')) {
      return await response.blob()
    }
    return null
  } catch {
    return null
  }
}

export async function deleteAvatar(): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/delete_avatar' })
}

export interface ChangePasswordRequest {
  old_password: string
  new_password: string
}

export async function changePassword(oldPassword: string, newPassword: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/change_password', data: { old_password: oldPassword, new_password: newPassword } })
}

export async function updateFeatureOrder(featureOrder: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/update_feature_order', data: { feature_order: featureOrder } })
}

export async function browseFolders(path?: string): Promise<BrowseResponse> {
  const data = path ? { path } : {}
  return request({ method: 'POST', url: '/system/browse', data })
}

export async function browseFiles(path?: string): Promise<FileBrowseResponse> {
  const data = path ? { path } : {}
  return request({ method: 'POST', url: '/file/browse', data })
}

export async function fetchDownloadFile(path: string, signal?: AbortSignal): Promise<Response> {
  const response = await fetch('/api/file/download', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ path }),
    credentials: 'include',
    signal,
  })
  const contentType = response.headers.get('Content-Type') || ''
  if (contentType.includes('application/json')) {
    const data = await response.json()
    if (data.fail_code === 'NOT_LOGGED_IN') {
      handleNotLoggedIn()
    }
    throw new Error(data.fail_code || 'Download failed')
  }
  return response
}

export interface CreateFolderRequest {
  parent_path?: string
  name: string
}

export interface DeleteRequest {
  path: string
}

export async function createFolder(parentPath: string | undefined, name: string): Promise<ApiResponse> {
  const data: CreateFolderRequest = { name }
  if (parentPath) {
    data.parent_path = parentPath
  }
  return requestWithSuccess({ method: 'POST', url: '/file/create_folder', data })
}

export async function deleteFile(path: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/file/delete', data: { path } })
}

export interface UploadStartResponse {
  success: boolean
  uploads?: Array<{ id: string; file: string }>
  fail_code?: string
  existing_files?: string[]
}

export interface UploadStartRequest {
  files: string[]
}

export async function uploadStart(files: string[]): Promise<UploadStartResponse> {
  return request({ method: 'POST', url: '/file/upload_start', data: { files }, skipErrorMessage: true, rawResponse: true })
}

export async function uploadChunk(
  uploadId: string,
  offset: number,
  chunk: Blob,
  onProgress?: (loaded: number, total: number) => void,
  signal?: AbortSignal
): Promise<ApiResponse> {
  const formData = new FormData()
  formData.append('upload_id', uploadId)
  formData.append('offset', offset.toString())
  formData.append('chunk', chunk)
  return request({
    method: 'POST',
    url: '/file/upload_chunk',
    data: formData,
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: 60000,
    skipErrorMessage: true,
    rawResponse: true,
    onUploadProgress: onProgress ? (e) => {
      if (e.total) {
        onProgress(e.loaded, e.total)
      }
    } : undefined,
    signal,
  })
}

export async function uploadComplete(uploadId: string): Promise<ApiResponse> {
  return request({ method: 'POST', url: '/file/upload_complete', data: { upload_id: uploadId }, skipErrorMessage: true, rawResponse: true })
}

export async function uploadCancel(uploadId: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/file/upload_cancel', data: { upload_id: uploadId }, skipErrorMessage: true })
}

export async function fetchDownloadFolder(path: string, signal?: AbortSignal): Promise<Response> {
  const response = await fetch('/api/file/download_folder', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ path }),
    credentials: 'include',
    signal,
  })
  const contentType = response.headers.get('Content-Type') || ''
  if (contentType.includes('application/json')) {
    const data = await response.json()
    if (data.fail_code === 'NOT_LOGGED_IN') {
      handleNotLoggedIn()
    }
    throw new Error(data.fail_code || 'Download failed')
  }
  return response
}

export interface MoveRequest {
  files: string[]
  current_path?: string
  target_path: string
}

export interface MoveResponse extends ApiResponse {
  conflict_files?: string[]
  failed_files?: string[]
}

export async function moveFiles(files: string[], currentPath?: string, targetPath?: string): Promise<MoveResponse> {
  const data: MoveRequest = { files, target_path: targetPath || '' }
  if (currentPath) {
    data.current_path = currentPath
  }
  return request({ method: 'POST', url: '/file/move', data, skipErrorMessage: true })
}

export interface BatchDeleteRequest {
  files: string[]
  current_path?: string
}

export async function batchDeleteFiles(files: string[], currentPath?: string): Promise<ApiResponse<{ failed_files?: string[] }>> {
  const data: BatchDeleteRequest = { files }
  if (currentPath) {
    data.current_path = currentPath
  }
  return request<ApiResponse<{ failed_files?: string[] }>>({ method: 'POST', url: '/file/batch_delete', data, skipErrorMessage: true, rawResponse: true })
}

export async function renameFile(path: string, newName: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/file/rename', data: { path, new_name: newName } })
}

export interface BackupRuleListItem {
  id: string
  name: string
  storage_type: string
  local_path: string
  cycle: string
  backup_time: Record<string, unknown>
  status: string
  next_backup_time: string | null
  last_backup_time: string | null
  created_at: string | null
}

export interface BackupRuleDetail {
  id: string
  name: string
  storage_type: string
  storage_config: {
    address: string
    username: string
    path: string
  }
  local_path: string
  encrypted: boolean
  cycle: string
  backup_time: Record<string, unknown>
  status: string
  last_backup_time: string | null
  created_at: string | null
}

export interface CreateBackupRuleRequest {
  name: string
  storage_type: string
  storage_config: {
    address: string
    username: string
    password: string
    path: string
  }
  local_path: string
  encrypted: boolean
  backup_password?: string
  cycle: string
  backup_time: Record<string, unknown>
}

export interface UpdateBackupRuleRequest {
  id: string
  name: string
  storage_type: string
  storage_config: {
    address: string
    username: string
    password: string
    path: string
  }
  local_path: string
  encrypted: boolean
  backup_password?: string
  cycle: string
  backup_time: Record<string, unknown>
}

export async function listBackupRules(): Promise<BackupRuleListItem[]> {
  return request({ method: 'POST', url: '/backup/list' })
}

export async function getBackupRule(id: string): Promise<BackupRuleDetail> {
  return request({ method: 'POST', url: '/backup/get', data: { id } })
}

export async function createBackupRule(data: CreateBackupRuleRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/backup/create', data })
}

export async function updateBackupRule(data: UpdateBackupRuleRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/backup/update', data })
}

export async function deleteBackupRule(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/backup/delete', data: { id } })
}

export interface StartBackupRequest {
  rule_id: string
  mode: 'full' | 'cleanup_only'
}

export interface StartBackupResponse {
  task_id: string
}

export async function startBackup(data: StartBackupRequest): Promise<StartBackupResponse> {
  return request({ method: 'POST', url: '/backup/start', data, timeout: 60000 })
}

export interface CancelBackupRequest {
  rule_id: string
}

export async function cancelBackup(data: CancelBackupRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/backup/cancel', data })
}

export interface BackupProgressItem {
  name: string
  status: string
  uploaded_bytes: number
  total_bytes: number
  error?: string
}

export interface BackupProgressResponse {
  is_running: boolean
  phase: 'backup' | 'cleanup' | null
  sub_phase: 'scanning' | null
  pending_items: BackupProgressItem[]
  total_count: number
  scanned_bytes: number
}

export interface GetBackupProgressRequest {
  rule_id: string
}

export async function getBackupProgress(data: GetBackupProgressRequest): Promise<BackupProgressResponse> {
  return request({ method: 'POST', url: '/backup/progress', data })
}

export interface BackupLogItem {
  id: string
  rule_id: string
  mode: 'full' | 'cleanup_only'
  status: 'completed' | 'failed' | 'cancelled' | 'interrupted'
  started_at: string
  finished_at: string | null
  backup_success_count: number
  backup_fail_count: number
  cleanup_deleted_count: number
  fail_reason: string | null
}

export interface BackupLogsResponse {
  total: number
  page: number
  page_size: number
  items: BackupLogItem[]
}

export interface GetBackupLogsRequest {
  rule_id: string
  page: number
  page_size: number
}

export async function getBackupLogs(data: GetBackupLogsRequest): Promise<BackupLogsResponse> {
  return request({ method: 'POST', url: '/backup/logs', data })
}

// ==================== 恢复相关接口 ====================

export interface CheckRestoreTargetRequest {
  local_path: string
}

export interface CheckRestoreTargetResponse {
  is_empty: boolean
  file_count: number
  files: string[]
}

export interface StartRestoreRequest {
  storage_type: string
  storage_config: {
    address: string
    username: string
    password: string
    path: string
  }
  encrypted: boolean
  backup_password?: string
  local_path: string
}

export interface StartRestoreResponse {
  task_id?: string
  success?: boolean
  fail_code?: string
  message?: string
}

export interface RestorePendingItem {
  name: string
  status: string
  total_bytes: number
  downloaded_bytes: number
  error: string | null
}

export interface RestoreProgressResponse {
  is_running: boolean
  downloading_items: RestorePendingItem[]
  failed_items: RestorePendingItem[]
  pending_count: number
  total_count: number
  success_count: number
  downloaded_bytes: number
}

export interface GetRestoreProgressRequest {
  task_id: string
}

export interface CancelRestoreRequest {
  task_id: string
}

export interface RetryRestoreFileRequest {
  task_id: string
  file_path: string
}

export async function checkRestoreTarget(localPath: string): Promise<CheckRestoreTargetResponse> {
  return request({ method: 'POST', url: '/restore/check', data: { local_path: localPath } })
}

export async function startRestore(data: StartRestoreRequest): Promise<StartRestoreResponse> {
  return request({ method: 'POST', url: '/restore/start', data, skipErrorMessage: true, timeout: 60000 })
}

export async function getRestoreProgress(taskId: string): Promise<RestoreProgressResponse> {
  return request({ method: 'POST', url: '/restore/progress', data: { task_id: taskId } })
}

export async function cancelRestore(taskId: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/restore/cancel', data: { task_id: taskId } })
}

export async function retryRestoreFile(taskId: string, filePath: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/restore/retry_file', data: { task_id: taskId, file_path: filePath } })
}

// ==================== 回收站相关接口 ====================

export interface RecycleBinItem {
  id: string
  original_path: string
  original_name: string
  is_directory: boolean
  file_size: number
  deleted_at: string
}

export interface RecycleBinListResponse {
  success: boolean
  fail_code?: string
  data?: {
    items: RecycleBinItem[]
    total: number
    page: number
    page_size: number
  }
}

export interface RecycleBinListRequest {
  page?: number
  page_size?: number
}

export interface BatchRestoreConflictItem {
  id: string
  original_path: string
  original_name: string
  is_directory: boolean
}

export async function getRecycleBinList(data?: RecycleBinListRequest): Promise<RecycleBinListResponse> {
  return request<RecycleBinListResponse>({ method: 'POST', url: '/recycle/list', data })
}

export async function restoreRecycleItem(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/recycle/restore', data: { id } })
}

export async function batchRestoreRecycleItems(ids: string[]) {
  return request<{ success: boolean; fail_code?: string; data?: { conflict_items?: { original_path: string; original_name: string; is_directory: boolean }[] } }>({
    method: 'POST', url: '/recycle/batch_restore', data: { ids }, skipErrorMessage: true, rawResponse: true,
  })
}

export async function deleteRecycleItem(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/recycle/delete', data: { id } })
}

export async function batchDeleteRecycleItems(ids: string[]) {
  return request<{
    success: boolean
    fail_code?: string
    data?: { failed_paths?: string[] }
  }>({
    method: 'POST',
    url: '/recycle/batch_delete',
    data: { ids },
    skipErrorMessage: true,
    rawResponse: true,
  })
}

export async function emptyRecycleBin(): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/recycle/empty', data: {} })
}

export interface VaultListItem {
  id: string
  name: string
  description: string
  path: string
  filename: string
  created_at: string | null
  updated_at: string | null
}

export interface VaultListResponse {
  vaults: VaultListItem[]
}

export interface CreateVaultRequest {
  name: string
  description?: string
  path: string
  filename: string
  file_data: string
}

export interface CreateVaultResponse {
  success: boolean
  fail_code?: string
  id?: string
}

export interface UpdateVaultRequest {
  id: string
  file_data?: string
}

export interface ImportVaultRequest {
  name: string
  description?: string
  file_path: string
}

export interface ImportVaultResponse {
  success: boolean
  fail_code?: string
  id?: string
}

export async function listVaults(): Promise<VaultListResponse> {
  const response = await request<VaultListResponse>({ method: 'POST', url: '/vault/list' })
  return response
}

export async function createVault(data: CreateVaultRequest): Promise<CreateVaultResponse> {
  return request<CreateVaultResponse>({ method: 'POST', url: '/vault/create', data, skipErrorMessage: true, rawResponse: true })
}

export async function updateVault(data: UpdateVaultRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/vault/update', data })
}

export interface UpdateVaultMetaRequest {
  id: string
  name?: string
  description?: string
}

export async function updateVaultMeta(data: UpdateVaultMetaRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/vault/update_meta', data })
}

export async function deleteVault(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/vault/delete', data: { id } })
}

export async function importVault(data: ImportVaultRequest): Promise<ImportVaultResponse> {
  return request<ImportVaultResponse>({ method: 'POST', url: '/vault/import', data, skipErrorMessage: true, rawResponse: true })
}

export async function uploadSingleFile(filePath: string, fileData: string): Promise<ApiResponse> {
  const binaryStr = atob(fileData)
  const bytes = new Uint8Array(binaryStr.length)
  for (let i = 0; i < binaryStr.length; i++) {
    bytes[i] = binaryStr.charCodeAt(i)
  }
  const blob = new Blob([bytes])
  const formData = new FormData()
  formData.append('path', filePath)
  formData.append('file', blob, 'vault.dat')
  return requestWithSuccess({ method: 'POST', url: '/vault/upload_single', data: formData, headers: { 'Content-Type': 'multipart/form-data' } })
}

// ==================== 笔记本相关接口 ====================

export interface NotebookItem {
  id: string
  name: string
  description: string
  path: string
  encrypted: boolean
  created_at: string | null
  updated_at: string | null
}

export interface NotebookListResponse {
  success: boolean
  notebooks: NotebookItem[]
}

export interface CreateNotebookRequest {
  name: string
  description?: string
  path: string
  encrypted?: boolean
  signature?: string
}

export interface CreateNotebookResponse {
  success: boolean
  fail_code?: string
  id?: string
}

export interface OpenNotebookRequest {
  name: string
  description?: string
  path: string
  encrypted?: boolean
}

export interface UpdateNotebookRequest {
  id: string
  name: string
  description?: string
}

export interface DeleteNotebookRequest {
  id: string
}

export interface ReadNoteRequest {
  notebook_id: string
  path: string
}

export interface ReadNoteResponse {
  success: boolean
  content: string
  hash: string
}

export interface ReadNoteErrorResponse {
  success: false
  fail_code: string
}

export interface SaveNoteRequest {
  notebook_id: string
  path: string
  content: string
  hash?: string
}

export interface SaveNoteResponse {
  success: boolean
  fail_code?: string
  hash?: string
  server_content?: string
  server_hash?: string
}

export interface SaveConflictRequest {
  notebook_id: string
  path: string
  content: string
}

export interface SaveConflictResponse {
  success: boolean
  fail_code?: string
  conflict_path?: string
  hash?: string
}

export interface FileTreeNode {
  name: string
  path: string
  is_dir: boolean
  children?: FileTreeNode[]
}

export interface FileTreeRequest {
  notebook_id: string
}

export interface FileTreeResponse {
  success: boolean
  tree: FileTreeNode[]
}

export interface RenameNoteRequest {
  notebook_id: string
  old_path: string
  new_name: string
}

export interface RenameNoteResponse {
  success: boolean
  fail_code?: string
  new_path?: string
}

export interface MoveNoteRequest {
  notebook_id: string
  source_path: string
  target_folder: string
}

export interface MoveNoteResponse {
  success: boolean
  fail_code?: string
  new_path?: string
}

export interface AttachmentTokenRequest {
  notebook_id: string
  key?: string
}

export interface AttachmentTokenResponse {
  success: boolean
  fail_code?: string
  token?: string
  expires_in?: number
}

export async function listNotebooks(): Promise<NotebookListResponse> {
  return request({ method: 'POST', url: '/notebook/list' })
}

export async function createNotebook(data: CreateNotebookRequest): Promise<CreateNotebookResponse> {
  return request({ method: 'POST', url: '/notebook/create', data, skipErrorMessage: true, rawResponse: true })
}

export async function openNotebook(data: OpenNotebookRequest): Promise<CreateNotebookResponse> {
  return request({ method: 'POST', url: '/notebook/open', data, skipErrorMessage: true, rawResponse: true })
}

export async function updateNotebook(data: UpdateNotebookRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/notebook/update', data })
}

export async function deleteNotebook(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/notebook/delete', data: { id } })
}

export async function readNote(data: ReadNoteRequest): Promise<ReadNoteResponse | ReadNoteErrorResponse> {
  return request({ method: 'POST', url: '/notebook/read_note', data, skipErrorMessage: true, rawResponse: true })
}

export async function saveNote(data: SaveNoteRequest): Promise<SaveNoteResponse> {
  return request({ method: 'POST', url: '/notebook/save_note', data, skipErrorMessage: true, rawResponse: true })
}

export async function createNotebookFolder(notebookId: string, path: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/notebook/create_folder', data: { notebook_id: notebookId, path } })
}

export async function saveConflict(data: SaveConflictRequest): Promise<SaveConflictResponse> {
  return request({ method: 'POST', url: '/notebook/save_conflict', data, skipErrorMessage: true, rawResponse: true })
}

export async function getFileTree(data: FileTreeRequest): Promise<FileTreeResponse> {
  return request({ method: 'POST', url: '/notebook/file_tree', data })
}

export async function renameNote(data: RenameNoteRequest): Promise<RenameNoteResponse> {
  return request({ method: 'POST', url: '/notebook/rename', data, skipErrorMessage: true, rawResponse: true })
}

export async function moveNote(data: MoveNoteRequest): Promise<MoveNoteResponse> {
  return request({ method: 'POST', url: '/notebook/move', data, skipErrorMessage: true, rawResponse: true })
}

export async function getAttachmentToken(data: AttachmentTokenRequest): Promise<AttachmentTokenResponse> {
  return request({ method: 'POST', url: '/notebook/attachment_token', data, skipErrorMessage: true, rawResponse: true })
}

export function getAttachmentUrl(notebookId: string, path: string, token: string): string {
  const params = new URLSearchParams({ path, notebook_id: notebookId, token })
  return `/api/notebook/attachment?${params.toString()}`
}

export async function uploadNotebookAttachment(notebookId: string, path: string, file: File): Promise<ApiResponse & { path?: string }> {
  const formData = new FormData()
  formData.append('notebook_id', notebookId)
  formData.append('path', path)
  formData.append('file', file)
  return requestWithSuccess({
    method: 'POST',
    url: '/notebook/upload_attachment',
    data: formData,
    headers: { 'Content-Type': 'multipart/form-data' },
  })
}

export interface BatchDeleteNotebookRequest {
  notebook_id: string
  paths: string[]
}

export interface BatchDeleteNotebookResponse extends ApiResponse {
  failed_paths?: string[]
}

export async function batchDeleteNotebookFiles(data: BatchDeleteNotebookRequest): Promise<BatchDeleteNotebookResponse> {
  return requestWithSuccess({ method: 'POST', url: '/notebook/batch_delete', data })
}

export async function deleteNotebookFolder(notebookId: string, path: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/notebook/delete_folder', data: { notebook_id: notebookId, path } })
}

// ==================== 笔记搜索接口 ====================

export interface SearchNotesRequest {
  keyword: string
  notebook_id?: string
}

export interface SearchMatch {
  line_number: number
  content: string
}

export interface SearchResultItem {
  notebook_id: string
  notebook_name: string
  note_path: string
  title: string
  title_matched: boolean
  matches: SearchMatch[]
  match_count: number
  modified: string | null
}

export interface SearchNotesResponse {
  success: boolean
  results: SearchResultItem[]
}

export async function searchNotes(data: SearchNotesRequest): Promise<SearchNotesResponse> {
  return request({ method: 'POST', url: '/notebook/search', data })
}

// ==================== 分享相关接口 ====================

export interface ShareItem {
  id: string
  file_name: string
  file_path?: string
  is_directory: boolean
  share_code: string
  expire_type: string
  expire_at?: string | null
  max_downloads?: number | null
  download_count: number
  share_mode: string
  has_password: boolean
  status: string
  created_at: string
}

export interface ShareInfoResponse {
  success: boolean
  file_name?: string
  file_size?: number
  is_directory?: boolean
  share_mode?: string
  need_password?: boolean
  password_salt?: string
  expire_type?: string
  expire_at?: string | null
  max_downloads?: number | null
  download_count?: number
  created_at?: string
  fail_code?: string
}

export interface CreateShareRequest {
  file_path: string
  expire_type: string
  expire_at?: string | null
  max_downloads?: number | null
  share_mode: string
  password?: string | null
}

export interface CreateShareResponse {
  success: boolean
  share_code?: string
  share_url?: string
  direct_url?: string
  fail_code?: string
}

export async function getShareInfo(data: { share_code: string }): Promise<ShareInfoResponse> {
  return request({ method: 'POST', url: '/share/info', data, skipErrorMessage: true, rawResponse: true })
}

export interface GetDownloadTokenResponse {
  success: boolean
  download_token?: string
  fail_code?: string
}

export async function getShareDownloadToken(data: { share_code: string; password_hash?: string }): Promise<GetDownloadTokenResponse> {
  return request({ method: 'POST', url: '/share/get_download_token', data, skipErrorMessage: true, rawResponse: true })
}

export async function createShare(data: CreateShareRequest): Promise<CreateShareResponse> {
  return request({ method: 'POST', url: '/share/create', data, skipErrorMessage: true, rawResponse: true })
}

export interface GetShareByPathResponse {
  success: boolean
  share?: ShareItem | null
  fail_code?: string
}

export interface ListSharesResponse {
  success: boolean
  shares?: ShareItem[]
  fail_code?: string
}

export async function getShareByPath(data: { file_path: string }): Promise<GetShareByPathResponse> {
  return request({ method: 'POST', url: '/share/get_by_path', data, skipErrorMessage: true, rawResponse: true })
}

export async function listShares(): Promise<ListSharesResponse> {
  return request({ method: 'POST', url: '/share/list', skipErrorMessage: true, rawResponse: true })
}

export async function deleteShares(data: { ids: string[] }): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/share/delete', data })
}

export interface WebDavConfigItem {
  id: string
  dav_path: string
  access_path: string
  permission: string
  url: string
  global_access: boolean
  created_at: string
  updated_at: string
}

export interface ListWebDavConfigResponse {
  success: boolean
  configs: WebDavConfigItem[]
  fail_code?: string
}

export interface CreateWebDavConfigRequest {
  dav_path: string
  access_path: string
  password: string
  permission: string
  global_access: boolean
}

export interface UpdateWebDavConfigRequest {
  id: string
  dav_path: string
  access_path: string
  password?: string
  permission: string
  global_access: boolean
}

export async function listWebDavConfigs(): Promise<ListWebDavConfigResponse> {
  return request({ method: 'POST', url: '/webdav/list', skipErrorMessage: true, rawResponse: true })
}

export async function createWebDavConfig(data: CreateWebDavConfigRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/webdav/create', data })
}

export async function updateWebDavConfig(data: UpdateWebDavConfigRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/webdav/update', data })
}

export async function deleteWebDavConfig(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/webdav/delete', data: { id } })
}

// ==================== 系统设置接口 ====================

export interface SystemSettingsResponse {
  success: boolean
  fail_code?: string
  system_name: string
  session_timeout: number
  notebook_fulltext_search: boolean
  has_logo: boolean
}

export interface UpdateSystemSettingsRequest {
  system_name: string
  session_timeout: number
  notebook_fulltext_search: boolean
}

export async function getSystemSettings(): Promise<SystemSettingsResponse> {
  return request({ method: 'POST', url: '/system/get_settings', data: {} })
}

export async function updateSystemSettings(data: UpdateSystemSettingsRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/system/update_settings', data })
}

export async function rebuildNotebookIndex(): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/system/rebuild_notebook_index', data: {} })
}

export async function uploadSystemLogo(file: File): Promise<ApiResponse> {
  const formData = new FormData()
  formData.append('logo', file)
  const response = await fetch('/api/system/upload_logo', {
    method: 'POST',
    body: formData,
    credentials: 'include',
  })
  return response.json()
}

export async function deleteSystemLogo(): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/system/delete_logo', data: {} })
}

export default api
