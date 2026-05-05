import { ElMessage as _ElMessage } from 'element-plus'
import i18n from '@/i18n'

interface TestMessage {
  type: string
  key: string | null
  message: string
}

if (import.meta.env.DEV) {
  ;(window as any).__test_messages = [] as TestMessage[]
  ;(window as any).__test_clear_messages = () => {
    ;(window as any).__test_messages = []
  }
}

function record(type: string, key: string | null, message: string) {
  if (import.meta.env.DEV) {
    ;(window as any).__test_messages.push({ type, key, message })
  }
}

function resolve(msg: any): { key: string | null; message: string } {
  if (typeof msg === 'object' && msg !== null && '__key' in msg) {
    const message = i18n.global.t(msg.__key, msg.__params)
    return { key: msg.__key, message }
  }
  const message = typeof msg === 'string' ? msg : msg?.message ?? String(msg)
  return { key: null, message }
}

function handler(opts: any) {
  const { key, message } = resolve(opts)
  const type = typeof opts === 'string' ? 'info' : (opts?.type ?? 'info')
  record(type, key, message)
  return _ElMessage(opts)
}

interface KeyedMessage { __key: string; __params?: Record<string, unknown> }
type MessageArg = string | KeyedMessage | any

const _success = _ElMessage.success as (msg: MessageArg) => ReturnType<typeof _ElMessage.success>
const _error = _ElMessage.error as (msg: MessageArg) => ReturnType<typeof _ElMessage.error>
const _warning = _ElMessage.warning as (msg: MessageArg) => ReturnType<typeof _ElMessage.warning>
const _info = _ElMessage.info as (msg: MessageArg) => ReturnType<typeof _ElMessage.info>

handler.success = (msg: MessageArg) => { const { key, message } = resolve(msg); record('success', key, message); return _success(message) }
handler.error = (msg: MessageArg) => { const { key, message } = resolve(msg); record('error', key, message); return _error(message) }
handler.warning = (msg: MessageArg) => { const { key, message } = resolve(msg); record('warning', key, message); return _warning(message) }
handler.info = (msg: MessageArg) => { const { key, message } = resolve(msg); record('info', key, message); return _info(message) }

export const ElMessage = handler as {
  (opts: any): ReturnType<typeof _ElMessage>
  success: (msg: MessageArg) => ReturnType<typeof _success>
  error: (msg: MessageArg) => ReturnType<typeof _error>
  warning: (msg: MessageArg) => ReturnType<typeof _warning>
  info: (msg: MessageArg) => ReturnType<typeof _info>
}
