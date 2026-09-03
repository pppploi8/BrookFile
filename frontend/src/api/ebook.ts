import { request, requestWithSuccess, type ApiResponse } from './system'

export type EbookFormat = 'txt' | 'epub' | 'pdf'
export type ScanStatus = 'pending' | 'scanning' | 'ready' | 'failed'
export type SourceStatus = 'ok' | 'file_not_found' | 'content_changed'

export interface EbookConfigResponse extends ApiResponse {
  ebook_path: string | null
  ebook_enabled: boolean
  ebook_db_status: string
}

export interface ShelfBook {
  id: string
  title: string
  format: EbookFormat
  file_size: number
  sha256: string
  category_id: string | null
  added_at: string
  scan_status: ScanStatus
  has_preview: boolean
}

export interface ShelfCategory {
  id: string
  name: string
  sort_order: number
}

export interface ShelfListResponse extends ApiResponse {
  books: ShelfBook[]
  categories: ShelfCategory[]
}

export interface ShelfAddedBook {
  id: string
  title: string
  format: EbookFormat
}

export interface ShelfDuplicateBook {
  path: string
  title: string
}

export interface ShelfAddResponse extends ApiResponse {
  added: ShelfAddedBook[]
  duplicates: ShelfDuplicateBook[]
  batch_id: string | null
}

export interface RelinkResponse extends ApiResponse {
  batch_id: string | null
}

export interface DiscoverFile {
  path: string
  format: EbookFormat
  title: string
  file_size: number
}

export interface DiscoverResponse extends ApiResponse {
  files: DiscoverFile[]
}

export interface ScanProgressResponse extends ApiResponse {
  is_running: boolean
  total: number
  completed: number
  failed: number
  current_book: string | null
}

export interface CategoryListResponse extends ApiResponse {
  categories: ShelfCategory[]
}

export interface CategoryAddResponse extends ApiResponse {
  id: string
}

export interface ChapterItem {
  chapter_no: number
  title: string | null
  location: string
}

export interface BookMetaResponse extends ApiResponse {
  book: ShelfBook & {
    source_path: string
    txt_encoding: string | null
    txt_cache_filename: string | null
    preview_filename: string | null
  }
  chapters: ChapterItem[]
  source_status: SourceStatus
}

export interface ProgressSlot {
  slot: number
  content_coord: string
  summary: string | null
  updated_at: string
}

export interface ProgressGetResponse extends ApiResponse {
  slots: ProgressSlot[]
}

export interface BookmarkItem {
  id: string
  content_coord: string
  summary: string | null
  created_at: string
}

export interface BookmarkListResponse extends ApiResponse {
  bookmarks: BookmarkItem[]
}

export interface BookmarkAddResponse extends ApiResponse {
  id: string
}

export async function ebookGetConfig(): Promise<EbookConfigResponse> {
  return request({ method: 'POST', url: '/ebook/config/get' })
}

export async function ebookSetConfig(ebookPath: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/config/set', data: { ebook_path: ebookPath } })
}

export async function ebookShelfList(): Promise<ShelfListResponse> {
  return request({ method: 'POST', url: '/ebook/shelf/list' })
}

export async function ebookShelfAdd(items: { path: string }[]): Promise<ShelfAddResponse> {
  return request({ method: 'POST', url: '/ebook/shelf/add', data: { items } })
}

export async function ebookShelfRemove(bookId: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/shelf/remove', data: { book_id: bookId } })
}

export async function ebookShelfRelink(bookId: string, newPath?: string): Promise<RelinkResponse> {
  return request({ method: 'POST', url: '/ebook/shelf/relink', data: { book_id: bookId, new_path: newPath } })
}

export async function ebookScanDiscover(path: string): Promise<DiscoverResponse> {
  return request({ method: 'POST', url: '/ebook/scan/discover', data: { path } })
}

export async function ebookScanProgress(batchId: string): Promise<ScanProgressResponse> {
  return request({ method: 'POST', url: '/ebook/scan/progress', data: { batch_id: batchId } })
}

export async function ebookCategoryList(): Promise<CategoryListResponse> {
  return request({ method: 'POST', url: '/ebook/category/list' })
}

export async function ebookCategoryAdd(name: string): Promise<CategoryAddResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/category/add', data: { name } })
}

export async function ebookCategoryRename(id: string, name: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/category/rename', data: { id, name } })
}

export async function ebookCategoryRemove(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/category/remove', data: { id } })
}

export async function ebookCategoryMoveBook(bookId: string, categoryId: string | null): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/category/move-book', data: { book_id: bookId, category_id: categoryId } })
}

export async function ebookBookMeta(bookId: string): Promise<BookMetaResponse> {
  return request({ method: 'POST', url: '/ebook/book/meta', data: { book_id: bookId } })
}

export async function ebookBookDownload(bookId: string): Promise<ArrayBuffer> {
  const resp = await fetch('/api/ebook/book/download', {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ book_id: bookId }),
  })
  if (!resp.ok) throw new Error('DOWNLOAD_FAILED')
  const ct = resp.headers.get('content-type') || ''
  if (ct.includes('application/json')) {
    const text = await resp.text()
    try {
      const j = JSON.parse(text)
      if (!j.success) throw new Error(j.fail_code || 'DOWNLOAD_FAILED')
    } catch {
      throw new Error('DOWNLOAD_FAILED')
    }
  }
  return resp.arrayBuffer()
}

export async function ebookPreviewGet(bookId: string): Promise<Blob> {
  const resp = await fetch('/api/ebook/preview/get', {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ book_id: bookId }),
  })
  if (!resp.ok) throw new Error('PREVIEW_FAILED')
  return resp.blob()
}

export interface ShelfCoverResponse extends ApiResponse {}

// Upload a client-rendered cover image (data URL) for a book.
export async function ebookShelfCover(bookId: string, imageDataUrl: string): Promise<ShelfCoverResponse> {
  return request({ method: 'POST', url: '/ebook/shelf/cover', data: { book_id: bookId, image: imageDataUrl } })
}

export async function ebookProgressSave(bookId: string, slot: number, contentCoord: string, summary: string | null): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/progress/save', data: { book_id: bookId, slot, content_coord: contentCoord, summary } })
}

export async function ebookProgressGet(bookId: string): Promise<ProgressGetResponse> {
  return request({ method: 'POST', url: '/ebook/progress/get', data: { book_id: bookId } })
}

export async function ebookBookmarkAdd(bookId: string, contentCoord: string, summary: string | null): Promise<BookmarkAddResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/bookmark/add', data: { book_id: bookId, content_coord: contentCoord, summary } })
}

export async function ebookBookmarkList(bookId: string): Promise<BookmarkListResponse> {
  return request({ method: 'POST', url: '/ebook/bookmark/list', data: { book_id: bookId } })
}

export async function ebookBookmarkRemove(id: string): Promise<ApiResponse> {
  return requestWithSuccess({ method: 'POST', url: '/ebook/bookmark/remove', data: { id } })
}

export async function fetchTestPdf(): Promise<ArrayBuffer> {
  const resp = await fetch('/api/ebook/test_pdf', {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
  })
  const buf = await resp.arrayBuffer()
  const ct = resp.headers.get('content-type') || ''
  if (ct.includes('application/json')) {
    const text = new TextDecoder().decode(buf)
    try {
      const j = JSON.parse(text)
      if (!j.success) throw new Error(j.fail_code || 'EBOOK_TEST_PDF_FAILED')
    } catch {
      throw new Error('EBOOK_TEST_PDF_FAILED')
    }
  }
  return buf
}
