import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  readNote as apiReadNote,
  saveNote as apiSaveNote,
  saveConflict as apiSaveConflict,
} from '@/api/system'
import type { SaveNoteResponse, SaveConflictResponse } from '@/api/system'
import { useNotebookStore } from './notebook'
import { useCryptoStore } from './crypto'

export interface NoteState {
  content: string
  hash: string | null
  path: string
  title: string
  notebookId: string
  isDirty: boolean
  isSaving: boolean
  isLoading: boolean
}

export const useNoteStore = defineStore('note', () => {
  const currentNote = ref<NoteState | null>(null)

  async function openNote(notebookId: string, path: string, encrypted: boolean, title?: string) {
    const displayTitle = title || path.replace(/\.md$/, '').split('/').pop() || ''
    currentNote.value = {
      content: '',
      hash: null,
      path,
      title: displayTitle,
      notebookId,
      isDirty: false,
      isSaving: false,
      isLoading: true,
    }

    const res = await apiReadNote({ notebook_id: notebookId, path })
    if ('success' in res && res.success && 'content' in res && 'hash' in res) {
      let content = res.content
      if (encrypted) {
        const cryptoStore = useCryptoStore()
        if (cryptoStore.isUnlocked(notebookId)) {
          try {
            content = await cryptoStore.decryptContent(notebookId, path, res.content)
          } catch {
            // decryption failed, show raw content
          }
        }
      }
      if (currentNote.value && currentNote.value.notebookId === notebookId && currentNote.value.path === path) {
        currentNote.value.content = content
        currentNote.value.hash = res.hash
        currentNote.value.isLoading = false
      }
    } else {
      if (currentNote.value && currentNote.value.notebookId === notebookId && currentNote.value.path === path) {
        currentNote.value.isLoading = false
      }
    }
  }

  async function saveCurrentNote(encrypted: boolean): Promise<SaveNoteResponse> {
    if (!currentNote.value) {
      return { success: true }
    }

    const note = currentNote.value
    note.isSaving = true

    let contentToSave = note.content
    let pathToSave = note.path

    if (encrypted) {
      const cryptoStore = useCryptoStore()
      if (!cryptoStore.isUnlocked(note.notebookId)) {
        note.isSaving = false
        return { success: false, fail_code: 'ENCRYPTED_NOTEBOOK' }
      }
      try {
        contentToSave = await cryptoStore.encryptContent(note.notebookId, note.path, note.content)
      } catch {
        note.isSaving = false
        return { success: false, fail_code: 'INTERNAL_ERROR' }
      }
    }

    const isNewNote = note.hash === null
    const res = await apiSaveNote({
      notebook_id: note.notebookId,
      path: pathToSave,
      content: contentToSave,
      hash: note.hash || undefined,
    })

    if (res.success) {
      note.hash = res.hash || null
      note.isDirty = false
      if (isNewNote) {
        const notebookStore = useNotebookStore()
        await notebookStore.fetchFileTree(note.notebookId)
      }
    }

    note.isSaving = false
    return res
  }

  async function saveConflictFile(notebookId: string, path: string, content: string): Promise<SaveConflictResponse> {
    return apiSaveConflict({ notebook_id: notebookId, path, content })
  }

  function updateContent(content: string) {
    if (currentNote.value) {
      if (currentNote.value.content === content) return
      currentNote.value.content = content
      currentNote.value.isDirty = true
    }
  }

  function closeNote() {
    currentNote.value = null
  }

  function resetState() {
    currentNote.value = null
  }

  return {
    currentNote,
    openNote,
    saveCurrentNote,
    saveConflictFile,
    updateContent,
    closeNote,
    resetState,
  }
})
