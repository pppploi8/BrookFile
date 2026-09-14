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

export async function request<T>(config: Parameters<typeof api.request>[0] & { skipErrorMessage?: boolean; rawResponse?: boolean }): Promise<T> {
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

export async function requestWithSuccess<T extends ApiResponse>(config: Parameters<typeof api.request>[0] & { skipErrorMessage?: boolean }): Promise<T> {
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
  ebook_path?: string
  ai_chat_path?: string
  ebook_enabled?: boolean
  ebook_db_status?: string
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

export async function setEbookPath(path: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/set_ebook_path', data: { ebook_path: path } })
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
  if (!response.ok) {
    const data = await response.json().catch(() => null)
    if (data?.fail_code === 'NOT_LOGGED_IN') {
      handleNotLoggedIn()
    }
    throw new Error(data?.fail_code || 'Download failed')
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

export async function uploadStart(files: string[], signal?: AbortSignal): Promise<UploadStartResponse> {
  return request({ method: 'POST', url: '/file/upload_start', data: { files }, skipErrorMessage: true, rawResponse: true, signal })
}

export interface UploadChunkResponse extends ApiResponse {
  // 仅在 INVALID_OFFSET 时返回：服务端当前已写入的字节数，用于对齐偏移后续传
  uploaded_bytes?: number
}

export async function uploadChunk(
  uploadId: string,
  offset: number,
  chunk: Blob,
  onProgress?: (loaded: number, total: number) => void,
  signal?: AbortSignal
): Promise<UploadChunkResponse> {
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

export async function uploadComplete(uploadId: string, signal?: AbortSignal): Promise<ApiResponse> {
  return request({ method: 'POST', url: '/file/upload_complete', data: { upload_id: uploadId }, skipErrorMessage: true, rawResponse: true, signal })
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
  if (!response.ok) {
    const data = await response.json().catch(() => null)
    if (data?.fail_code === 'NOT_LOGGED_IN') {
      handleNotLoggedIn()
    }
    throw new Error(data?.fail_code || 'Download failed')
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

export interface ListWebDavCorsResponse {
  success: boolean
  origins: string[]
  fail_code?: string
}

export async function listWebDavCors(): Promise<ListWebDavCorsResponse> {
  return request({ method: 'POST', url: '/webdav/cors/list', skipErrorMessage: true, rawResponse: true })
}

export async function saveWebDavCors(origins: string[]): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/webdav/cors/save', data: { origins } })
}

// ==================== 系统设置接口 ====================

export interface SystemSettingsResponse {
  success: boolean
  fail_code?: string
  system_name: string
  session_timeout_days: number
  max_login_devices: number
  notebook_fulltext_search: boolean
  has_logo: boolean
}

export interface UpdateSystemSettingsRequest {
  system_name: string
  session_timeout_days: number
  max_login_devices: number
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

// ==================== AI 配置接口 ====================

export interface AiModelItem {
  id: string
  model_id: string
  supports_vision: boolean
  context_length: number
  max_output_tokens: number
  created_at: string
  updated_at: string
}

export interface AiProviderItem {
  id: string
  name: string
  provider_type: string
  base_url: string
  proxy: string
  has_api_key: boolean
  created_at: string
  updated_at: string
  models: AiModelItem[]
}

export interface AiProviderPreset {
  type_id: string
  name: string
  default_base_url: string
  requires_api_key: boolean
}

export interface ListProviderPresetsResponse {
  success: boolean
  presets: AiProviderPreset[]
}

export interface ListAiProvidersResponse {
  success: boolean
  providers: AiProviderItem[]
}

export interface AiNewModelInput {
  model_id: string
  supports_vision?: boolean
  context_length?: number
  max_output_tokens?: number
}

export interface CreateAiProviderRequest {
  name: string
  provider_type: string
  base_url?: string
  api_key: string
  proxy?: string
  models?: AiNewModelInput[]
}

export interface CreateAiProviderResponse {
  success: boolean
  provider_id: string
}

export interface AiModelEditInput extends AiNewModelInput {
  id?: string
}

export interface UpdateAiProviderRequest {
  id: string
  name: string
  provider_type: string
  base_url?: string
  api_key?: string
  proxy?: string
  models?: AiModelEditInput[]
}

export interface FetchAiModelsResponse {
  success: boolean
  models: string[]
}

export async function listAiProviders(): Promise<ListAiProvidersResponse> {
  return request({ method: 'POST', url: '/ai/provider/list', data: {}, skipErrorMessage: true, rawResponse: true })
}

export async function listAiProviderPresets(): Promise<ListProviderPresetsResponse> {
  return request({ method: 'POST', url: '/ai/provider/presets', data: {} })
}

export async function createAiProvider(data: CreateAiProviderRequest): Promise<CreateAiProviderResponse> {
  return request({ method: 'POST', url: '/ai/provider/create', data })
}

export async function updateAiProvider(data: UpdateAiProviderRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ai/provider/update', data })
}

export async function deleteAiProvider(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ai/provider/delete', data: { id } })
}

export async function fetchAiModels(
  provider_type: string,
  base_url?: string,
  api_key?: string,
  proxy?: string,
  provider_id?: string,
): Promise<FetchAiModelsResponse> {
  return request({
    method: 'POST',
    url: '/ai/provider/fetch_models',
    data: { provider_type, base_url: base_url || '', api_key: api_key || '', proxy: proxy || '', provider_id },
    skipErrorMessage: true,
  })
}

export async function deleteAiModel(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ai/model/delete', data: { id } })
}

// ==================== 登录设备接口 ====================

export interface SessionInfo {
  id: string
  device_name: string
  user_agent: string
  ip_address: string
  created_at: number
  last_access_time: number
  is_current: boolean
}

export interface ListSessionsResponse {
  success: boolean
  fail_code?: string
  sessions?: SessionInfo[]
}

export interface UpdateSessionNameRequest {
  session_id: string
  device_name: string
}

export interface RevokeSessionRequest {
  session_id: string
}

export async function listSessions(): Promise<ListSessionsResponse> {
  return request({ method: 'POST', url: '/session/list', data: {} })
}

export async function updateSessionName(data: UpdateSessionNameRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/session/update_name', data })
}

export async function revokeSession(data: RevokeSessionRequest): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/session/revoke', data })
}


export interface AiChatMeta {
  id: string
  biz_type: string
  biz_id: string
  title: string
  created_at: string
  updated_at: string
}

export interface AiChatMessage {
  id: string
  role: 'user' | 'assistant' | 'tool' | 'system' | 'compact'
  content: string
  reasoning?: string
  images: string[]
  tool_calls?: unknown
  tool_call_id?: string
  name?: string
  model_key?: string
  created_at: string
}

export interface ListAiChatsResponse extends ApiResponse {
  chats?: AiChatMeta[]
}

export interface CreateAiChatResponse extends ApiResponse {
  chat_id?: string
}

export interface GetAiChatMessagesResponse extends ApiResponse {
  messages?: AiChatMessage[]
  has_more_before?: boolean
  has_more_after?: boolean
}

export async function setAiChatPath(path: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/user/set_ai_chat_path', data: { ai_chat_path: path } })
}

export async function listAiChats(bizType: string, bizId: string): Promise<ListAiChatsResponse> {
  return request({ method: 'POST', url: '/ai/chat/list', data: { biz_type: bizType, biz_id: bizId } })
}

export async function createAiChat(bizType: string, bizId: string): Promise<CreateAiChatResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ai/chat/create', data: { biz_type: bizType, biz_id: bizId } })
}

export async function renameAiChat(bizType: string, chatId: string, title: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ai/chat/rename', data: { biz_type: bizType, chat_id: chatId, title } })
}

export async function deleteAiChat(bizType: string, chatId: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ai/chat/delete', data: { biz_type: bizType, chat_id: chatId } })
}

// 回传 AI 工具的页面查询应答（data 格式由 kind 定义，空串表示应答失败）
export async function postAiPageQueryResult(requestId: string, data: string): Promise<void> {
  await requestWithSuccess({
    method: 'POST',
    url: '/ai/chat/page_query_result',
    data: { request_id: requestId, data },
  })
}

export async function getAiChatMessages(
  bizType: string,
  chatId: string,
  options?: { beforeId?: string; afterId?: string; limit?: number },
): Promise<GetAiChatMessagesResponse> {
  return request({
    method: 'POST',
    url: '/ai/chat/messages',
    data: {
      biz_type: bizType,
      chat_id: chatId,
      before_id: options?.beforeId,
      after_id: options?.afterId,
      limit: options?.limit,
    },
  })
}

export interface AiChatStreamHandlers {
  onUserMessage?: (msg: AiChatMessage) => void
  onReasoning?: (content: string) => void
  onDelta?: (content: string) => void
  onToolStart?: (name: string, args: string) => void
  onToolEnd?: (name: string, ok: boolean) => void
  onPageQuery?: (requestId: string, kind: string, params: Record<string, unknown>) => Promise<void>
  onCompactStart?: (tokensBefore: number) => void
  onCompactEnd?: (ok: boolean) => void
  onDone?: (messageId: string) => void
  /** detail 为上游返回的原始报错（如 HTTP 状态码与错误消息），拿不到时为空 */
  onError?: (failCode: string, detail?: string) => void
}

export async function streamAiChatSend(
  params: { bizType: string; chatId: string; modelKey: string; content: string; images: string[]; thinking?: string },
  handlers: AiChatStreamHandlers,
  signal?: AbortSignal,
): Promise<void> {
  const resp = await fetch('/api/ai/chat/send', {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      biz_type: params.bizType,
      chat_id: params.chatId,
      model_key: params.modelKey,
      content: params.content,
      images: params.images,
      thinking: params.thinking || undefined,
    }),
    signal,
  })
  if (!resp.ok || !resp.body) {
    handlers.onError?.('AI_CALL_FAILED')
    return
  }
  const contentType = resp.headers.get('content-type') || ''
  if (!contentType.includes('text/event-stream')) {
    let failCode = 'AI_CALL_FAILED'
    try {
      const json = await resp.json()
      if (json?.fail_code) failCode = json.fail_code
    } catch {
      // 保持默认错误码
    }
    handlers.onError?.(failCode)
    return
  }
  const reader = resp.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, '\n')
    let pos: number
    while ((pos = buffer.indexOf('\n\n')) !== -1) {
      const frame = buffer.slice(0, pos)
      buffer = buffer.slice(pos + 2)
      let event = 'message'
      const dataLines: string[] = []
      for (const line of frame.split('\n')) {
        if (line.startsWith('event:')) event = line.slice(6).trim()
        else if (line.startsWith('data:')) dataLines.push(line.slice(5).trim())
      }
      if (!dataLines.length) continue
      let data: any
      try {
        data = JSON.parse(dataLines.join('\n'))
      } catch {
        continue
      }
      if (event === 'user_message') handlers.onUserMessage?.(data)
      else if (event === 'reasoning') handlers.onReasoning?.(data.content ?? '')
      else if (event === 'delta') handlers.onDelta?.(data.content ?? '')
      else if (event === 'tool_start') handlers.onToolStart?.(data.name ?? '', data.arguments ?? '')
      else if (event === 'tool_end') handlers.onToolEnd?.(data.name ?? '', !!data.ok)
      else if (event === 'page_query') await handlers.onPageQuery?.(data.request_id ?? '', data.kind ?? '', (data.params ?? {}) as Record<string, unknown>)
      else if (event === 'compact_start') handlers.onCompactStart?.(data.tokens_before ?? 0)
      else if (event === 'compact_end') handlers.onCompactEnd?.(!!data.ok)
      else if (event === 'done') handlers.onDone?.(data.message_id ?? '')
      else if (event === 'error') handlers.onError?.(data.fail_code ?? 'AI_CALL_FAILED', typeof data.detail === 'string' ? data.detail : undefined)
    }
  }
}

export default api
