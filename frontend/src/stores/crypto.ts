import { defineStore } from 'pinia'
import { shallowRef } from 'vue'
import CryptoJS from 'crypto-js'

interface FileTreeNode {
  name: string
  path: string
  is_dir: boolean
  children?: FileTreeNode[]
}

interface SignatureFile {
  salt: string
  iv: string
  rounds: number
  signature: string
}

const VERIFY_STRING = 'BROOKFILE_NOTEBOOK_VERIFY'

export function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  let binary = ''
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]!)
  }
  return btoa(binary)
}

export function base64ToArrayBuffer(base64: string): ArrayBuffer {
  let binary: string
  try {
    binary = atob(base64)
  } catch {
    throw new Error('Invalid base64 string')
  }
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes.buffer
}

function toUrlSafeBase64(base64: string): string {
  return base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

function fromUrlSafeBase64(urlSafe: string): string {
  let base64 = urlSafe.replace(/-/g, '+').replace(/_/g, '/')
  while (base64.length % 4 !== 0) {
    base64 += '='
  }
  return base64
}

export interface CryptoBackend {
  deriveKeyAndIV(password: string, salt: Uint8Array, rounds: number): Promise<{ key: Uint8Array; iv: Uint8Array }>
  encrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<ArrayBuffer>
  decrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<ArrayBuffer>
  hmacSha256(key: Uint8Array, data: Uint8Array): Promise<Uint8Array>
}

function toBuffer(source: Uint8Array): ArrayBuffer {
  if (source.buffer instanceof ArrayBuffer) {
    return source.buffer.slice(source.byteOffset, source.byteOffset + source.byteLength)
  }
  return new Uint8Array(source).buffer
}

class WebCryptoBackend implements CryptoBackend {
  async deriveKeyAndIV(password: string, salt: Uint8Array, rounds: number): Promise<{ key: Uint8Array; iv: Uint8Array }> {
    const keyMaterial = await crypto.subtle.importKey(
      'raw',
      new TextEncoder().encode(password),
      'PBKDF2',
      false,
      ['deriveBits'],
    )
    const derivedBits = await crypto.subtle.deriveBits(
      { name: 'PBKDF2', salt: toBuffer(salt), iterations: rounds, hash: 'SHA-256' },
      keyMaterial,
      384,
    )
    const derivedBytes = new Uint8Array(derivedBits)
    return { key: derivedBytes.slice(0, 32), iv: derivedBytes.slice(32, 48) }
  }

  async encrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<ArrayBuffer> {
    const cryptoKey = await crypto.subtle.importKey('raw', toBuffer(key), { name: 'AES-CBC' }, false, ['encrypt'])
    return crypto.subtle.encrypt({ name: 'AES-CBC', iv: toBuffer(iv) }, cryptoKey, toBuffer(data))
  }

  async decrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<ArrayBuffer> {
    const cryptoKey = await crypto.subtle.importKey('raw', toBuffer(key), { name: 'AES-CBC' }, false, ['decrypt'])
    return crypto.subtle.decrypt({ name: 'AES-CBC', iv: toBuffer(iv) }, cryptoKey, toBuffer(data))
  }

  async hmacSha256(key: Uint8Array, data: Uint8Array): Promise<Uint8Array> {
    const cryptoKey = await crypto.subtle.importKey('raw', toBuffer(key), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign'])
    const signature = await crypto.subtle.sign('HMAC', cryptoKey, toBuffer(data))
    return new Uint8Array(signature)
  }
}

function wordArrayToUint8Array(wordArray: CryptoJS.lib.WordArray): Uint8Array {
  const words = wordArray.words
  const sigBytes = wordArray.sigBytes
  const bytes = new Uint8Array(sigBytes)
  for (let i = 0; i < sigBytes; i++) {
    bytes[i] = (words[i >>> 2]! >>> (24 - (i % 4) * 8)) & 0xff
  }
  return bytes
}

function uint8ArrayToWordArray(bytes: Uint8Array): CryptoJS.lib.WordArray {
  const words: number[] = []
  for (let i = 0; i < bytes.length; i += 4) {
    words.push(
      ((bytes[i]! << 24) |
        ((bytes[i + 1] ?? 0) << 16) |
        ((bytes[i + 2] ?? 0) << 8) |
        (bytes[i + 3] ?? 0)) >>> 0,
    )
  }
  return CryptoJS.lib.WordArray.create(words, bytes.length)
}

class CryptoJSBackend implements CryptoBackend {
  async deriveKeyAndIV(password: string, salt: Uint8Array, rounds: number): Promise<{ key: Uint8Array; iv: Uint8Array }> {
    const saltWordArray = uint8ArrayToWordArray(salt)
    const derived = CryptoJS.PBKDF2(password, saltWordArray, {
      keySize: 384 / 32,
      iterations: rounds,
      hasher: CryptoJS.algo.SHA256,
    })
    const fullBytes = wordArrayToUint8Array(derived)
    return { key: fullBytes.slice(0, 32), iv: fullBytes.slice(32, 48) }
  }

  async encrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<ArrayBuffer> {
    const keyWordArray = uint8ArrayToWordArray(key)
    const ivWordArray = uint8ArrayToWordArray(iv)
    const dataWordArray = uint8ArrayToWordArray(data)
    const cipherParams = CryptoJS.AES.encrypt(dataWordArray, keyWordArray, {
      iv: ivWordArray,
      mode: CryptoJS.mode.CBC,
      padding: CryptoJS.pad.Pkcs7,
    })
    const ctBytes = wordArrayToUint8Array(cipherParams.ciphertext)
    return ctBytes.buffer instanceof ArrayBuffer ? ctBytes.buffer : new Uint8Array(ctBytes).buffer
  }

  async decrypt(key: Uint8Array, iv: Uint8Array, data: Uint8Array): Promise<ArrayBuffer> {
    const keyWordArray = uint8ArrayToWordArray(key)
    const ivWordArray = uint8ArrayToWordArray(iv)
    const cipherParams = CryptoJS.lib.CipherParams.create({
      ciphertext: uint8ArrayToWordArray(data),
    })
    const decrypted = CryptoJS.AES.decrypt(cipherParams, keyWordArray, {
      iv: ivWordArray,
      mode: CryptoJS.mode.CBC,
      padding: CryptoJS.pad.Pkcs7,
    })
    const ptBytes = wordArrayToUint8Array(decrypted)
    return ptBytes.buffer instanceof ArrayBuffer ? ptBytes.buffer : new Uint8Array(ptBytes).buffer
  }

  async hmacSha256(key: Uint8Array, data: Uint8Array): Promise<Uint8Array> {
    const keyWordArray = uint8ArrayToWordArray(key)
    const dataWordArray = uint8ArrayToWordArray(data)
    const hmac = CryptoJS.HmacSHA256(dataWordArray, keyWordArray)
    return wordArrayToUint8Array(hmac)
  }
}

export const backend: CryptoBackend = typeof crypto !== 'undefined' && crypto.subtle
  ? new WebCryptoBackend()
  : new CryptoJSBackend()

export const useCryptoStore = defineStore('crypto', () => {
  const keyCache = shallowRef<Map<string, Uint8Array>>(new Map())
  const ivCache = shallowRef<Map<string, Uint8Array>>(new Map())
  const fileTreeCache = shallowRef<Map<string, FileTreeNode[]>>(new Map())

  function setKeyCache(notebookId: string, key: Uint8Array) {
    const newMap = new Map(keyCache.value)
    newMap.set(notebookId, key)
    keyCache.value = newMap
  }

  function setIvCache(notebookId: string, iv: Uint8Array) {
    const newMap = new Map(ivCache.value)
    newMap.set(notebookId, iv)
    ivCache.value = newMap
  }

  async function deriveKeyAndIV(password: string, salt: string, rounds: number): Promise<{ key: Uint8Array; iv: Uint8Array }> {
    const saltBuffer = new Uint8Array(base64ToArrayBuffer(salt))
    return backend.deriveKeyAndIV(password, saltBuffer, rounds)
  }

  async function generateSignatureFile(password: string): Promise<SignatureFile> {
    const saltBytes = crypto.getRandomValues(new Uint8Array(16))
    const salt = arrayBufferToBase64(toBuffer(saltBytes))
    const rounds = 100000
    const { key, iv } = await backend.deriveKeyAndIV(password, saltBytes, rounds)
    const encoded = new TextEncoder().encode(VERIFY_STRING)
    const encrypted = await backend.encrypt(key, iv, encoded)
    return {
      salt,
      iv: arrayBufferToBase64(toBuffer(iv)),
      rounds,
      signature: arrayBufferToBase64(encrypted),
    }
  }

  async function verifyPassword(signatureContent: string, password: string): Promise<{ valid: boolean; key: Uint8Array; iv: Uint8Array }> {
    const sig: SignatureFile = JSON.parse(signatureContent)
    const saltBuffer = new Uint8Array(base64ToArrayBuffer(sig.salt))
    const { key, iv } = await backend.deriveKeyAndIV(password, saltBuffer, sig.rounds)
    try {
      const decrypted = await backend.decrypt(key, iv, new Uint8Array(base64ToArrayBuffer(sig.signature)))
      const text = new TextDecoder().decode(decrypted)
      return { valid: text === VERIFY_STRING, key, iv }
    } catch {
      return { valid: false, key, iv }
    }
  }

  async function unlockNotebook(notebookId: string, password: string, signatureContent: string): Promise<boolean> {
    const { valid, key, iv } = await verifyPassword(signatureContent, password)
    if (valid) {
      setKeyCache(notebookId, key)
      setIvCache(notebookId, iv)
    }
    return valid
  }

  function lockNotebook(notebookId: string): void {
    const newKeys = new Map(keyCache.value)
    const newIvs = new Map(ivCache.value)
    const newTrees = new Map(fileTreeCache.value)
    newKeys.delete(notebookId)
    newIvs.delete(notebookId)
    newTrees.delete(notebookId)
    keyCache.value = newKeys
    ivCache.value = newIvs
    fileTreeCache.value = newTrees
  }

  function lockAll(): void {
    keyCache.value = new Map()
    ivCache.value = new Map()
    fileTreeCache.value = new Map()
  }

  function isUnlocked(notebookId: string): boolean {
    return keyCache.value.has(notebookId)
  }

  async function encryptContent(notebookId: string, _path: string, content: string): Promise<string> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    const iv = crypto.getRandomValues(new Uint8Array(16))
    const encoded = new TextEncoder().encode(content)
    const encrypted = await backend.encrypt(key, iv, encoded)
    const combined = new Uint8Array(iv.byteLength + encrypted.byteLength)
    combined.set(iv, 0)
    combined.set(new Uint8Array(encrypted), iv.byteLength)
    return arrayBufferToBase64(combined.buffer as ArrayBuffer)
  }

  async function decryptContent(notebookId: string, _path: string, encrypted: string): Promise<string> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    const encryptedBytes = new Uint8Array(base64ToArrayBuffer(encrypted))
    const iv = encryptedBytes.slice(0, 16)
    const ciphertext = encryptedBytes.slice(16)
    const decrypted = await backend.decrypt(key, iv, ciphertext)
    return new TextDecoder().decode(decrypted)
  }

  async function encryptPath(notebookId: string, path: string): Promise<string> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    const iv = ivCache.value.get(notebookId)
    if (!iv) throw new Error('Notebook is locked')
    const segments = path.split('/')
    const encryptedSegments: string[] = []
    for (const segment of segments) {
      if (segment.endsWith('.md')) {
        const namePart = segment.slice(0, -3)
        const encoded = new TextEncoder().encode(namePart)
        const cipher = await backend.encrypt(key, iv, encoded)
        encryptedSegments.push(toUrlSafeBase64(arrayBufferToBase64(cipher)) + '.md')
      } else {
        const encoded = new TextEncoder().encode(segment)
        const cipher = await backend.encrypt(key, iv, encoded)
        encryptedSegments.push(toUrlSafeBase64(arrayBufferToBase64(cipher)))
      }
    }
    return encryptedSegments.join('/')
  }

  async function decryptPath(notebookId: string, encryptedPath: string): Promise<string> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    const iv = ivCache.value.get(notebookId)
    if (!iv) throw new Error('Notebook is locked')
    const segments = encryptedPath.split('/')
    const decryptedSegments: string[] = []
    for (const segment of segments) {
      if (segment === 'attachment') {
        decryptedSegments.push(segment)
      } else if (segment.endsWith('.md')) {
        const base64 = fromUrlSafeBase64(segment.slice(0, -3))
        const decrypted = await backend.decrypt(key, iv, new Uint8Array(base64ToArrayBuffer(base64)))
        decryptedSegments.push(new TextDecoder().decode(decrypted) + '.md')
      } else {
        const base64 = fromUrlSafeBase64(segment)
        const decrypted = await backend.decrypt(key, iv, new Uint8Array(base64ToArrayBuffer(base64)))
        decryptedSegments.push(new TextDecoder().decode(decrypted))
      }
    }
    return decryptedSegments.join('/')
  }

  async function encryptAttachment(notebookId: string, data: ArrayBuffer): Promise<ArrayBuffer> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    const randomIv = crypto.getRandomValues(new Uint8Array(16))
    const encrypted = await backend.encrypt(key, randomIv, new Uint8Array(data))
    const combined = new Uint8Array(randomIv.byteLength + encrypted.byteLength)
    combined.set(randomIv, 0)
    combined.set(new Uint8Array(encrypted), randomIv.byteLength)
    return combined.buffer as ArrayBuffer
  }

  async function decryptAttachment(notebookId: string, encrypted: ArrayBuffer): Promise<ArrayBuffer> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    const encryptedBytes = new Uint8Array(encrypted)
    const randomIv = encryptedBytes.slice(0, 16)
    const ciphertext = encryptedBytes.slice(16)
    return backend.decrypt(key, randomIv, ciphertext)
  }

  async function exportKey(notebookId: string): Promise<string> {
    const key = keyCache.value.get(notebookId)
    if (!key) throw new Error('Notebook is locked')
    return arrayBufferToBase64(toBuffer(key))
  }

  function cacheFileTree(notebookId: string, tree: FileTreeNode[]): void {
    const newMap = new Map(fileTreeCache.value)
    newMap.set(notebookId, tree)
    fileTreeCache.value = newMap
  }

  async function searchEncryptedNotebooks(keyword: string): Promise<Array<{ notebookId: string; notePath: string; title: string }>> {
    const results: Array<{ notebookId: string; notePath: string; title: string }> = []
    const lowerKeyword = keyword.toLowerCase()
    for (const notebookId of keyCache.value.keys()) {
      const tree = fileTreeCache.value.get(notebookId)
      if (!tree) continue
      async function walk(nodes: FileTreeNode[]): Promise<void> {
        for (const node of nodes) {
          if (node.is_dir) {
            if (node.children) await walk(node.children)
          } else if (node.name.endsWith('.md')) {
            try {
              const decrypted = await decryptPath(notebookId, node.name.slice(0, -3))
              if (decrypted.toLowerCase().includes(lowerKeyword)) {
                results.push({ notebookId, notePath: node.path, title: decrypted })
              }
            } catch { /* skip undecryptable names */ }
          }
        }
      }
      await walk(tree)
    }
    return results
  }

  return {
    keyCache,
    ivCache,
    fileTreeCache,
    deriveKeyAndIV,
    generateSignatureFile,
    verifyPassword,
    unlockNotebook,
    lockNotebook,
    lockAll,
    isUnlocked,
    encryptContent,
    decryptContent,
    encryptPath,
    decryptPath,
    encryptAttachment,
    decryptAttachment,
    exportKey,
    cacheFileTree,
    searchEncryptedNotebooks,
  }
})
