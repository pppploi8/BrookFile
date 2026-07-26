import * as Y from 'yjs'

export type CollabStatus = 'connected' | 'syncing' | 'disconnected'

export class NoteCollabProvider {
  private ws: WebSocket | null = null
  private doc: Y.Doc
  private url: string
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private snapshotTimer: ReturnType<typeof setTimeout> | null = null
  private destroyed = false
  private reconnectAttempts = 0
  private receivedSnapshot = false
  private initialContent: string
  private initTimeout: ReturnType<typeof setTimeout> | null = null

  public onStatusChange: ((status: CollabStatus) => void) | null = null
  public ready: Promise<void>
  private resolveReady!: () => void

  constructor(notebookId: string, notePath: string, doc: Y.Doc, initialContent: string) {
    this.doc = doc
    this.initialContent = initialContent
    this.url = `/ws/note?notebook_id=${encodeURIComponent(notebookId)}&path=${encodeURIComponent(notePath)}`
    this.ready = new Promise((resolve) => { this.resolveReady = resolve })
    this.doc.on('update', this.handleLocalUpdate)
    this.connect()
  }

  private setStatus(status: CollabStatus) {
    this.onStatusChange?.(status)
  }

  private connect() {
    if (this.destroyed) return
    this.setStatus('syncing')

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}${this.url}`)
    ws.binaryType = 'arraybuffer'
    this.ws = ws

    ws.onopen = () => {
      this.reconnectAttempts = 0
      this.setStatus('connected')
      if (!this.receivedSnapshot) {
        this.initTimeout = setTimeout(() => {
          if (!this.receivedSnapshot && !this.destroyed) {
            this.seedFromContent()
            this.resolveReady()
          }
        }, 300)
      }
    }

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        const data = new Uint8Array(event.data)
        if (data.length > 1) {
          if (data[0] === 0x02) {
            const text = new TextDecoder().decode(data.slice(1))
            const yText = this.doc.getText('content')
            this.doc.transact(() => {
              yText.delete(0, yText.length)
              yText.insert(0, text)
            }, 'remote')
            this.scheduleSnapshot()
          } else if (data[0] === 0x00) {
            this.receivedSnapshot = true
            if (this.initTimeout) { clearTimeout(this.initTimeout); this.initTimeout = null }
            Y.applyUpdate(this.doc, data.slice(1), 'remote')
            this.resolveReady()
          }
        }
      }
    }

    ws.onclose = () => {
      if (this.destroyed) return
      this.setStatus('disconnected')
      this.scheduleReconnect()
    }

    ws.onerror = () => {
      ws.close()
    }
  }

  private seedFromContent() {
    const yText = this.doc.getText('content')
    if (yText.length === 0 && this.initialContent) {
      this.doc.transact(() => {
        yText.insert(0, this.initialContent)
      })
      this.scheduleSnapshot()
    }
  }

  private scheduleReconnect() {
    if (this.destroyed) return
    this.reconnectAttempts++
    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts - 1), 30000)
    this.reconnectTimer = setTimeout(() => this.connect(), delay)
  }

  private handleLocalUpdate = (update: Uint8Array, origin: unknown) => {
    if (origin === 'remote') return
    if (this.ws?.readyState === WebSocket.OPEN) {
      const msg = new Uint8Array(1 + update.length)
      msg[0] = 0x00
      msg.set(update, 1)
      this.ws.send(msg)
    }
    this.scheduleSnapshot()
  }

  private scheduleSnapshot() {
    if (this.snapshotTimer) clearTimeout(this.snapshotTimer)
    this.snapshotTimer = setTimeout(() => this.sendSnapshot(), 2000)
  }

  private sendSnapshot() {
    if (this.ws?.readyState !== WebSocket.OPEN) return
    const yjsState = Y.encodeStateAsUpdate(this.doc)
    const yText = this.doc.getText('content')
    const payload = JSON.stringify({
      yjs_state: uint8ToBase64(yjsState),
      text: yText.toString(),
    })
    const jsonBytes = new TextEncoder().encode(payload)
    const msg = new Uint8Array(1 + jsonBytes.length)
    msg[0] = 0x01
    msg.set(jsonBytes, 1)
    this.ws.send(msg)
  }

  destroy() {
    this.destroyed = true
    if (this.snapshotTimer) clearTimeout(this.snapshotTimer)
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    if (this.initTimeout) clearTimeout(this.initTimeout)
    this.sendSnapshot()
    this.doc.off('update', this.handleLocalUpdate)
    if (this.ws) {
      this.ws.onclose = null
      this.ws.close()
      this.ws = null
    }
  }
}

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunkSize = 8192
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const chunk = bytes.slice(i, i + chunkSize)
    binary += String.fromCharCode(...chunk)
  }
  return btoa(binary)
}
