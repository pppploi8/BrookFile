import { computed, nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import {
  createAiChat,
  deleteAiChat,
  getAiChatMessages,
  listAiChats,
  listAiProviders,
  postAiPageQueryResult,
  renameAiChat,
  streamAiChatSend,
  type AiChatMessage,
  type AiChatMeta,
} from '@/api/system'

export interface UiMessage {
  id: string
  role: 'user' | 'assistant' | 'tool' | 'system' | 'compact'
  content: string
  images?: string[]
  reasoning?: string
  toolCalls?: unknown
  toolCallId?: string
  toolName?: string
  modelKey?: string
}

const WIN_SIZE = 100
const WIN_STEP = 50

export function useAiChat(
  bizType: string,
  bizId: () => string,
  options?: { pageQuery?: (kind: string, params: Record<string, unknown>) => Promise<string | null> },
) {
  const { t } = useI18n()

  const chats = ref<AiChatMeta[]>([])
  const activeChatId = ref('')
  const historyOpen = ref(false)
  const messages = ref<UiMessage[]>([])
  const hasMoreBefore = ref(false)
  const hasMoreAfter = ref(false)
  const loadingMessages = ref(false)
  const streaming = ref(false)
  const streamingId = ref<string | null>(null)
  const compacting = ref(false)
  const compactNotice = ref('')
  const sending = ref(false)
  const toolStatus = ref('')
  const input = ref('')
  const pending = ref<{ kind: 'image' | 'text'; value: string }[]>([])
  const modelKey = ref('')
  const modelOptions = ref<{ key: string; label: string; vision: boolean }[]>([])
  // 'default' 仅作 UI 哨兵值（el-select 空串会显示占位符），发送时不携带 thinking 参数
  const thinking = ref('default')
  const listEl = ref<HTMLElement | null>(null)
  const editingId = ref<string | null>(null)
  const editingTitle = ref('')

  const winStart = ref(0)
  const winEnd = ref(WIN_SIZE)

  const activeChat = computed(() => chats.value.find((c) => c.id === activeChatId.value))
  const visibleMessages = computed(() => messages.value.slice(winStart.value, winEnd.value))
  const canVision = computed(
    () => modelOptions.value.find((o) => o.key === modelKey.value)?.vision ?? false,
  )

  async function loadModels(): Promise<void> {
    try {
      const resp = await listAiProviders()
      const opts: { key: string; label: string; vision: boolean }[] = []
      for (const p of resp.providers ?? []) {
        for (const m of p.models ?? []) {
          opts.push({ key: m.id, label: `${p.name} / ${m.model_id}`, vision: m.supports_vision })
        }
      }
      modelOptions.value = opts
      if (opts.length) modelKey.value = opts[0]!.key
    } catch {
      // 模型列表拉取失败不阻塞面板
    }
  }

  async function loadChats(): Promise<void> {
    try {
      const resp = await listAiChats(bizType, bizId())
      chats.value = resp.chats ?? []
    } catch {
      chats.value = []
    }
  }

  function scrollToBottom(): void {
    requestAnimationFrame(() => {
      if (listEl.value) listEl.value.scrollTop = listEl.value.scrollHeight
    })
  }

  // 流式增量时使用：仅当用户在底部附近才跟随滚动，上翻阅读时不打断
  function followScroll(): void {
    const el = listEl.value
    if (!el) return
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 120) scrollToBottom()
  }

  function resetWindowToTail(): void {
    winStart.value = Math.max(0, messages.value.length - WIN_SIZE)
    winEnd.value = messages.value.length
  }

  // role=system 是首轮落盘的业务上下文提示、role=compact 是压缩摘要，仅回传给模型，前端不展示
  function toUiMessages(list: AiChatMessage[]): UiMessage[] {
    return list.filter((m) => m.role !== 'system' && m.role !== 'compact').map(toUiMessage)
  }

  function toUiMessage(m: AiChatMessage): UiMessage {
    return {
      id: m.id,
      role: m.role,
      content: m.content,
      images: m.images,
      reasoning: m.reasoning,
      toolCalls: m.tool_calls,
      toolCallId: m.tool_call_id,
      toolName: m.name,
      modelKey: m.model_key,
    }
  }

  async function openChat(chatId: string): Promise<void> {
    activeChatId.value = chatId
    historyOpen.value = false
    loadingMessages.value = true
    try {
      const resp = await getAiChatMessages(bizType, chatId, { limit: WIN_SIZE })
      messages.value = toUiMessages(resp.messages ?? [])
      hasMoreBefore.value = !!resp.has_more_before
      hasMoreAfter.value = false
      resetWindowToTail()
      scrollToBottom()
    } catch {
      messages.value = []
    } finally {
      loadingMessages.value = false
    }
  }

  async function reconcile(chatId: string): Promise<void> {
    try {
      const resp = await getAiChatMessages(bizType, chatId, { limit: WIN_SIZE })
      messages.value = toUiMessages(resp.messages ?? [])
      hasMoreBefore.value = !!resp.has_more_before
      hasMoreAfter.value = false
      resetWindowToTail()
      scrollToBottom()
    } catch {
      // 保持现状
    }
  }

  async function newChat(): Promise<void> {
    if (activeChat.value && messages.value.length === 0) {
      historyOpen.value = false
      return
    }
    try {
      const resp = await createAiChat(bizType, bizId())
      if (!resp.chat_id) return
      await loadChats()
      messages.value = []
      hasMoreBefore.value = false
      hasMoreAfter.value = false
      activeChatId.value = resp.chat_id
      historyOpen.value = false
      resetWindowToTail()
    } catch {
      // 失败提示由请求层统一处理
    }
  }

  async function switchChat(chatId: string): Promise<void> {
    await openChat(chatId)
  }

  function startRename(chat: AiChatMeta): void {
    editingId.value = chat.id
    editingTitle.value = chat.title
  }

  async function confirmRename(): Promise<void> {
    const id = editingId.value
    if (id === null) return
    const title = editingTitle.value.trim()
    editingId.value = null
    if (!title) return
    try {
      await renameAiChat(bizType, id, title)
      const chat = chats.value.find((c) => c.id === id)
      if (chat) chat.title = title
    } catch {
      // 失败提示由请求层统一处理
    }
  }

  function cancelRename(): void {
    editingId.value = null
  }

  async function removeChat(chatId: string): Promise<void> {
    try {
      await deleteAiChat(bizType, chatId)
      await loadChats()
      if (activeChatId.value === chatId) {
        const next = chats.value[0]
        if (next) await openChat(next.id)
        else {
          activeChatId.value = ''
          messages.value = []
        }
      }
    } catch {
      // 失败提示由请求层统一处理
    }
  }

  async function loadOlder(): Promise<void> {
    if (loadingMessages.value || !hasMoreBefore.value) return
    const first = messages.value[winStart.value]
    if (!first) return
    loadingMessages.value = true
    try {
      const el = listEl.value
      const anchorTop =
        el?.querySelector(`[data-idx="${first.id}"]`)?.getBoundingClientRect().top ?? 0
      const resp = await getAiChatMessages(bizType, activeChatId.value, {
        beforeId: first.id,
        limit: WIN_STEP,
      })
      const older = toUiMessages(resp.messages ?? [])
      messages.value = [...older, ...messages.value]
      winStart.value += older.length
      winEnd.value += older.length
      hasMoreBefore.value = !!resp.has_more_before
      await nextTick()
      const target = listEl.value?.querySelector(`[data-idx="${first.id}"]`)
      if (listEl.value && target) {
        listEl.value.scrollTop +=
          (target as HTMLElement).getBoundingClientRect().top - anchorTop
      }
    } catch {
      // 失败提示由请求层统一处理
    } finally {
      loadingMessages.value = false
    }
  }

  async function loadNewer(): Promise<void> {
    if (loadingMessages.value || !hasMoreAfter.value) return
    const last = messages.value[winEnd.value - 1]
    if (!last) return
    loadingMessages.value = true
    try {
      const el = listEl.value
      const anchorTop =
        el?.querySelector(`[data-idx="${last.id}"]`)?.getBoundingClientRect().top ?? 0
      const resp = await getAiChatMessages(bizType, activeChatId.value, {
        afterId: last.id,
        limit: WIN_STEP,
      })
      const newer = toUiMessages(resp.messages ?? [])
      messages.value = [...messages.value, ...newer]
      winEnd.value = messages.value.length
      winStart.value = Math.max(0, winEnd.value - WIN_SIZE)
      hasMoreAfter.value = !!resp.has_more_after
      await nextTick()
      const target = listEl.value?.querySelector(`[data-idx="${last.id}"]`)
      if (listEl.value && target) {
        listEl.value.scrollTop +=
          (target as HTMLElement).getBoundingClientRect().top - anchorTop
      }
    } catch {
      // 失败提示由请求层统一处理
    } finally {
      loadingMessages.value = false
    }
  }

  let abortController: AbortController | null = null

  function stopStreaming(): void {
    abortController?.abort()
  }

  async function send(captureNeedsVision: boolean): Promise<void> {
    if (sending.value || streaming.value) return
    const content = input.value.trim()
    if (!pending.value.length && !content) return
    if (!modelKey.value) return
    if (imagesPending() && captureNeedsVision && !canVision.value) return

    if (!activeChatId.value) {
      try {
        const resp = await createAiChat(bizType, bizId())
        if (!resp.chat_id) return
        await loadChats()
        activeChatId.value = resp.chat_id
      } catch {
        return
      }
    }

    const images = pending.value.filter((p) => p.kind === 'image').map((p) => p.value)
    const parts = pending.value
      .filter((p) => p.kind === 'text')
      .map((p) => `${t('reader.aiRefLabel')}\n${p.value}`)
    if (content) parts.push(content)

    sending.value = true
    streaming.value = true
    toolStatus.value = ''
    abortController = new AbortController()
    let assistantId = ''

    try {
      await streamAiChatSend(
        {
          bizType,
          chatId: activeChatId.value,
          modelKey: modelKey.value,
          content: parts.join('\n\n'),
          images,
          thinking: thinking.value === 'default' ? undefined : thinking.value,
        },
        {
          onUserMessage: (msg) => {
            messages.value.push(toUiMessage(msg))
            pending.value = []
            input.value = ''
            resetWindowToTail()
            scrollToBottom()
          },
          onReasoning: (delta) => {
            if (!assistantId) {
              const msg: UiMessage = {
                id: `streaming-${Date.now()}`,
                role: 'assistant',
                content: '',
                reasoning: delta,
                modelKey: modelKey.value,
              }
              assistantId = msg.id
              streamingId.value = msg.id
              messages.value.push(msg)
              winEnd.value = messages.value.length
            } else {
              const msg = messages.value.find((m) => m.id === assistantId)
              if (msg) msg.reasoning = (msg.reasoning ?? '') + delta
            }
            followScroll()
          },
          onDelta: (delta) => {
            if (!assistantId) {
              const msg: UiMessage = {
                id: `streaming-${Date.now()}`,
                role: 'assistant',
                content: delta,
                modelKey: modelKey.value,
              }
              assistantId = msg.id
              streamingId.value = msg.id
              messages.value.push(msg)
              winEnd.value = messages.value.length
            } else {
              const msg = messages.value.find((m) => m.id === assistantId)
              if (msg) msg.content += delta
            }
            followScroll()
          },
          onToolStart: (name, args) => {
            toolStatus.value = args ? `${name} ${args}` : name
            followScroll()
          },
          onToolEnd: () => {
            toolStatus.value = ''
          },
          onPageQuery: async (requestId, kind, params) => {
            let reply = ''
            try {
              reply = (await options?.pageQuery?.(kind, params)) ?? ''
            } catch {
              reply = ''
            }
            try {
              await postAiPageQueryResult(requestId, reply)
            } catch {
              // 回传失败由后端查询超时兜底
            }
          },
          onCompactStart: () => {
            compacting.value = true
            compactNotice.value = t('ai.contextCompacting')
            followScroll()
          },
          onCompactEnd: (ok) => {
            compacting.value = false
            compactNotice.value = ok ? t('ai.contextCompacted') : ''
            if (ok) followScroll()
          },
          onError: (failCode) => {
            ElMessage.error({ __key: `errors.${failCode}` })
          },
        },
        abortController.signal,
      )
    } catch {
      // 请求异常（含主动中断），随后从服务端对账
    } finally {
      abortController = null
      streaming.value = false
      streamingId.value = null
      sending.value = false
      toolStatus.value = ''
      compacting.value = false
      compactNotice.value = ''
      if (activeChatId.value) await reconcile(activeChatId.value)
      await loadChats()
    }
  }

  function imagesPending(): boolean {
    return pending.value.some((p) => p.kind === 'image')
  }

  function removePending(i: number): void {
    pending.value.splice(i, 1)
  }

  async function onCapture(capture?: () => Promise<string | null>, asImage?: boolean): Promise<void> {
    if (!capture || sending.value || streaming.value) return
    const result = await capture()
    if (!result) return
    pending.value.push({ kind: asImage ? 'image' : 'text', value: result })
  }

  function chatImageUrl(rel: string): string {
    return `/api/ai/chat/image?biz_type=${encodeURIComponent(bizType)}&path=${encodeURIComponent(rel)}`
  }

  async function init(): Promise<void> {
    await Promise.all([loadModels(), loadChats()])
    if (!activeChatId.value && chats.value.length) await openChat(chats.value[0]!.id)
  }

  function formatTime(ts: string): string {
    return ts.slice(5, 16)
  }

  return {
    chats,
    activeChat,
    activeChatId,
    historyOpen,
    messages,
    visibleMessages,
    hasMoreBefore,
    hasMoreAfter,
    loadingMessages,
    streaming,
    streamingId,
    compacting,
    compactNotice,
    sending,
    toolStatus,
    input,
    pending,
    modelKey,
    modelOptions,
    thinking,
    canVision,
    listEl,
    editingId,
    editingTitle,
    winStart,
    winEnd,
    loadModels,
    loadChats,
    openChat,
    newChat,
    switchChat,
    startRename,
    confirmRename,
    cancelRename,
    removeChat,
    loadOlder,
    loadNewer,
    send,
    stopStreaming,
    removePending,
    onCapture,
    chatImageUrl,
    scrollToBottom,
    init,
    formatTime,
    t,
  }
}

export type AiChatController = ReturnType<typeof useAiChat>
