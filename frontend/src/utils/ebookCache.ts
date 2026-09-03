const DB_NAME = 'brookfile-ebook'
const DB_VERSION = 1
const STORE_NAME = 'books'

const CACHE_KEY_SEP = '::'

function cacheKey(bookId: string, sha256: string): string {
  return `${bookId}${CACHE_KEY_SEP}${sha256}`
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME)
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

export async function cacheBookData(bookId: string, sha256: string, data: ArrayBuffer): Promise<void> {
  const db = await openDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite')
    tx.objectStore(STORE_NAME).put(data, cacheKey(bookId, sha256))
    tx.oncomplete = () => { db.close(); resolve() }
    tx.onerror = () => { db.close(); reject(tx.error) }
  })
}

export async function getCachedBookData(bookId: string, sha256: string): Promise<ArrayBuffer | null> {
  const db = await openDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly')
    const req = tx.objectStore(STORE_NAME).get(cacheKey(bookId, sha256))
    req.onsuccess = () => { db.close(); resolve(req.result ?? null) }
    req.onerror = () => { db.close(); reject(req.error) }
  })
}

export async function isBookCached(bookId: string, sha256: string): Promise<boolean> {
  const data = await getCachedBookData(bookId, sha256)
  return data !== null
}

export async function removeCachedBook(bookId: string): Promise<void> {
  const db = await openDb()
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite')
    const store = tx.objectStore(STORE_NAME)
    const range = IDBKeyRange.bound(`${bookId}${CACHE_KEY_SEP}`, `${bookId}${CACHE_KEY_SEP}\uffff`)
    store.delete(range)
    tx.oncomplete = () => { db.close(); resolve() }
    tx.onerror = () => { db.close(); reject(tx.error) }
  })
}

export async function downloadWithProgress(
  bookId: string,
  onProgress: (loaded: number, total: number) => void,
  signal?: AbortSignal,
): Promise<ArrayBuffer> {
  const resp = await fetch('/api/ebook/book/download', {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ book_id: bookId }),
    signal,
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

  const total = Number(resp.headers.get('content-length') || 0)
  const reader = resp.body?.getReader()
  if (!reader) {
    return resp.arrayBuffer()
  }

  const chunks: Uint8Array[] = []
  let loaded = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    chunks.push(value)
    loaded += value.length
    onProgress(loaded, total)
  }
  const merged = new Uint8Array(loaded)
  let offset = 0
  for (const chunk of chunks) {
    merged.set(chunk, offset)
    offset += chunk.length
  }
  return merged.buffer
}
