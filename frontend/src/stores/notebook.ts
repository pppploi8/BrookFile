import { defineStore } from 'pinia'
import { ref, reactive, computed } from 'vue'
import {
  listNotebooks as apiListNotebooks,
  createNotebook as apiCreateNotebook,
  openNotebook as apiOpenNotebook,
  updateNotebook as apiUpdateNotebook,
  deleteNotebook as apiDeleteNotebook,
  getFileTree as apiGetFileTree,
  renameNote as apiRenameNote,
  moveNote as apiMoveNote,
  uploadNotebookAttachment as apiUploadNotebookAttachment,
  batchDeleteNotebookFiles as apiBatchDeleteNotebookFiles,
  deleteNotebookFolder as apiDeleteNotebookFolder,
  getAttachmentToken as apiGetAttachmentToken,
} from '@/api/system'
import type {
  NotebookItem,
  CreateNotebookRequest,
  OpenNotebookRequest,
  FileTreeNode,
} from '@/api/system'
import { useCryptoStore } from './crypto'

interface CachedAttachmentToken {
  token: string
  expiresAt: number
}

export const useNotebookStore = defineStore('notebook', () => {
  const notebooks = ref<NotebookItem[]>([])
  const currentNotebookId = ref<string | null>(null)
  const loading = ref(false)
  const fileTreeMap = ref<Map<string, FileTreeNode[]>>(new Map())
  const fileTreeLoading = reactive(new Map<string, boolean>())
  const attachmentTokenCache = ref<Map<string, CachedAttachmentToken>>(new Map())

  const currentNotebook = computed(() => {
    if (!currentNotebookId.value) return null
    return notebooks.value.find(n => n.id === currentNotebookId.value) || null
  })

  const encryptedNotebooks = computed(() => notebooks.value.filter(n => n.encrypted))
  const normalNotebooks = computed(() => notebooks.value.filter(n => !n.encrypted))

  function setCurrentNotebook(id: string | null) {
    currentNotebookId.value = id
  }

  async function fetchNotebooks() {
    loading.value = true
    try {
      const res = await apiListNotebooks()
      notebooks.value = res.notebooks || []
    } finally {
      loading.value = false
    }
  }

  async function createNotebook(data: CreateNotebookRequest) {
    const res = await apiCreateNotebook(data)
    if (res.success && res.id) {
      await fetchNotebooks()
    }
    return res
  }

  async function openNotebook(data: OpenNotebookRequest) {
    const res = await apiOpenNotebook(data)
    if (res.success && res.id) {
      await fetchNotebooks()
    }
    return res
  }

  async function updateNotebookData(id: string, name: string, description: string) {
    await apiUpdateNotebook({ id, name, description })
    await fetchNotebooks()
  }

  async function removeNotebook(id: string) {
    const res = await apiDeleteNotebook(id)
    if (res.success) {
      fileTreeMap.value.delete(id)
      attachmentTokenCache.value.delete(id)
      fileTreeLoading.delete(id)
      await fetchNotebooks()
    }
    return res
  }

  async function fetchFileTree(notebookId: string) {
    fileTreeLoading.set(notebookId, true)
    try {
      const res = await apiGetFileTree({ notebook_id: notebookId })
      const tree = res.tree || []
      const nb = getNotebookById(notebookId)
      if (nb?.encrypted) {
        const cryptoStore = useCryptoStore()
        if (cryptoStore.isUnlocked(notebookId)) {
          cryptoStore.cacheFileTree(notebookId, JSON.parse(JSON.stringify(tree)))
          await decryptTreeNames(notebookId, tree)
        }
      }
      sortTree(tree)
      fileTreeMap.value.set(notebookId, tree)
      return tree
    } finally {
      fileTreeLoading.set(notebookId, false)
    }
  }

  async function decryptTreeNames(notebookId: string, nodes: FileTreeNode[]) {
    const cryptoStore = useCryptoStore()
    for (const node of nodes) {
      if (node.name !== 'attachment') {
        try {
          node.name = await cryptoStore.decryptPath(notebookId, node.name)
        } catch { /* skip undecryptable names */ }
      }
      if (node.children) {
        await decryptTreeNames(notebookId, node.children)
      }
    }
  }

  function sortTree(nodes: FileTreeNode[]) {
    nodes.sort((a, b) => {
      if (a.name === 'attachment') return -1
      if (b.name === 'attachment') return 1
      if (a.is_dir && !b.is_dir) return -1
      if (!a.is_dir && b.is_dir) return 1
      return a.name.localeCompare(b.name)
    })
    for (const node of nodes) {
      if (node.children) sortTree(node.children)
    }
  }

  async function renameNotePath(notebookId: string, oldPath: string, newName: string) {
    const res = await apiRenameNote({ notebook_id: notebookId, old_path: oldPath, new_name: newName })
    if (res.success) {
      await fetchFileTree(notebookId)
    }
    return res
  }

  async function moveNotePath(notebookId: string, sourcePath: string, targetFolder: string) {
    const res = await apiMoveNote({ notebook_id: notebookId, source_path: sourcePath, target_folder: targetFolder })
    if (res.success) {
      await fetchFileTree(notebookId)
    }
    return res
  }

  function clearFileTree(notebookId: string) {
    fileTreeMap.value.delete(notebookId)
  }

  function getNotebookById(id: string): NotebookItem | undefined {
    return notebooks.value.find(n => n.id === id)
  }

  async function uploadAttachment(notebookId: string, path: string, file: File) {
    return apiUploadNotebookAttachment(notebookId, path, file)
  }

  async function batchDeleteFiles(notebookId: string, paths: string[]) {
    const res = await apiBatchDeleteNotebookFiles({ notebook_id: notebookId, paths })
    if (res.success) {
      await fetchFileTree(notebookId)
    }
    return res
  }

  async function deleteFolder(notebookId: string, path: string) {
    const res = await apiDeleteNotebookFolder(notebookId, path)
    if (res.success) {
      await fetchFileTree(notebookId)
    }
    return res
  }

  async function getAttachmentToken(notebookId: string): Promise<string> {
    const cached = attachmentTokenCache.value.get(notebookId)
    if (cached && cached.expiresAt > Date.now()) {
      return cached.token
    }
    const nb = getNotebookById(notebookId)
    let key: string | undefined
    if (nb?.encrypted) {
      const cryptoStore = useCryptoStore()
      if (cryptoStore.isUnlocked(notebookId)) {
        key = await cryptoStore.exportKey(notebookId)
      }
    }
    const tokenRes = await apiGetAttachmentToken({ notebook_id: notebookId, key })
    if (tokenRes.success && tokenRes.token) {
      const expiresIn = tokenRes.expires_in ?? 3600
      const renewAhead = Math.max(expiresIn - 300, 60)
      attachmentTokenCache.value.set(notebookId, {
        token: tokenRes.token,
        expiresAt: Date.now() + renewAhead * 1000,
      })
      return tokenRes.token
    }
    return ''
  }

  function clearAttachmentToken(notebookId: string) {
    attachmentTokenCache.value.delete(notebookId)
  }

  function clearAllAttachmentTokens() {
    attachmentTokenCache.value.clear()
  }

  return {
    notebooks,
    currentNotebookId,
    currentNotebook,
    encryptedNotebooks,
    normalNotebooks,
    loading,
    fileTreeMap,
    fileTreeLoading,
    fetchNotebooks,
    createNotebook,
    openNotebook,
    updateNotebookData,
    removeNotebook,
    fetchFileTree,
    renameNotePath,
    moveNotePath,
    clearFileTree,
    getNotebookById,
    setCurrentNotebook,
    uploadAttachment,
    batchDeleteFiles,
    deleteFolder,
    getAttachmentToken,
    clearAttachmentToken,
    clearAllAttachmentTokens,
  }
})
