import { defineStore } from 'pinia'
import { ref } from 'vue'
import {
  ebookShelfList,
  ebookShelfAdd,
  ebookShelfRemove,
  ebookShelfRelink,
  ebookScanDiscover,
  ebookScanProgress,
  ebookCategoryAdd,
  ebookCategoryRename,
  ebookCategoryRemove,
  ebookCategoryMoveBook,
  ebookBookMeta,
  ebookProgressSave,
  ebookProgressGet,
  ebookBookmarkAdd,
  ebookBookmarkList,
  ebookBookmarkRemove,
  type ShelfBook,
  type ShelfCategory,
  type ScanProgressResponse,
} from '@/api/ebook'
import { removeCachedBook } from '@/utils/ebookCache'

export const useEbookStore = defineStore('ebook', () => {
  const books = ref<ShelfBook[]>([])
  const categories = ref<ShelfCategory[]>([])
  const loaded = ref(false)

  const cachedBookIds = ref<Set<string>>(new Set())

  async function loadShelf() {
    const resp = await ebookShelfList()
    if (resp.success) {
      books.value = resp.books
      categories.value = resp.categories
      loaded.value = true
    }
  }

  async function addBooks(items: { path: string }[]) {
    const resp = await ebookShelfAdd(items)
    if (resp.success) {
      await loadShelf()
    }
    return resp
  }

  async function removeBook(bookId: string) {
    await ebookShelfRemove(bookId)
    await removeCachedBook(bookId)
    cachedBookIds.value.delete(bookId)
    await loadShelf()
  }

  async function relinkBook(bookId: string, newPath?: string) {
    const resp = await ebookShelfRelink(bookId, newPath)
    await loadShelf()
    return resp
  }

  async function discoverDirectory(path: string) {
    return await ebookScanDiscover(path)
  }

  async function getScanProgress(batchId: string) {
    return await ebookScanProgress(batchId)
  }

  async function waitForScan(batchId: string, timeout = 60): Promise<ScanProgressResponse | null> {
    const deadline = Date.now() + timeout * 1000
    for (;;) {
      const resp = await ebookScanProgress(batchId)
      if (!resp.is_running) return resp
      if (Date.now() > deadline) return resp
      await new Promise(r => setTimeout(r, 500))
    }
  }

  async function addCategory(name: string) {
    const resp = await ebookCategoryAdd(name)
    if (resp.success) {
      await loadShelf()
    }
    return resp
  }

  async function renameCategory(id: string, name: string) {
    await ebookCategoryRename(id, name)
    await loadShelf()
  }

  async function removeCategory(id: string) {
    await ebookCategoryRemove(id)
    await loadShelf()
  }

  async function moveBook(bookId: string, categoryId: string | null) {
    await ebookCategoryMoveBook(bookId, categoryId)
    await loadShelf()
  }

  async function getBookMeta(bookId: string) {
    return await ebookBookMeta(bookId)
  }

  async function saveProgress(bookId: string, slot: number, contentCoord: string, summary: string | null) {
    await ebookProgressSave(bookId, slot, contentCoord, summary)
  }

  async function getProgress(bookId: string) {
    return await ebookProgressGet(bookId)
  }

  async function addBookmark(bookId: string, contentCoord: string, summary: string | null) {
    return await ebookBookmarkAdd(bookId, contentCoord, summary)
  }

  async function listBookmarks(bookId: string) {
    return await ebookBookmarkList(bookId)
  }

  async function removeBookmark(id: string) {
    await ebookBookmarkRemove(id)
  }

  function setCached(bookId: string, cached: boolean) {
    if (cached) cachedBookIds.value.add(bookId)
    else cachedBookIds.value.delete(bookId)
  }

  function isCached(bookId: string): boolean {
    return cachedBookIds.value.has(bookId)
  }

  return {
    books,
    categories,
    loaded,
    cachedBookIds,
    loadShelf,
    addBooks,
    removeBook,
    relinkBook,
    discoverDirectory,
    getScanProgress,
    waitForScan,
    addCategory,
    renameCategory,
    removeCategory,
    moveBook,
    getBookMeta,
    saveProgress,
    getProgress,
    addBookmark,
    listBookmarks,
    removeBookmark,
    setCached,
    isCached,
  }
})
