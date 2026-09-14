<script setup lang="ts">
import { onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import MarkdownIt from 'markdown-it'
import hljs from 'highlight.js/lib/common'
import 'highlight.js/styles/github.css'
import {
  Camera,
  ChatDotRound,
  Clock,
  Close,
  Delete,
  EditPen,
  Loading,
  Plus,
  Promotion,
  VideoPause,
} from '@element-plus/icons-vue'
import { useAiChat } from '@/composables/useAiChat'

const props = defineProps<{
  bizType: string
  bizId?: string
  capture?: () => Promise<string | null>
  visionCapture?: boolean
  pageQuery?: (kind: string, params: Record<string, unknown>) => Promise<string | null>
}>()

const { t } = useI18n()
const ai = useAiChat(props.bizType, () => props.bizId ?? '', { pageQuery: props.pageQuery })
// 模板 ref 需要接到 composable 的 listEl 上，否则其内部滚动逻辑拿不到元素
const listEl = ai.listEl
function setListEl(el: unknown): void {
  listEl.value = el instanceof HTMLElement ? el : null
}

const md: InstanceType<typeof MarkdownIt> = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
  highlight(code, lang) {
    const safeLang = lang ? lang.replace(/[^a-zA-Z0-9_-]/g, '').toLowerCase() : ''
    if (safeLang && hljs.getLanguage(safeLang)) {
      try {
        return `<pre><code class="hljs language-${safeLang}">${hljs.highlight(code, {
          language: safeLang,
          ignoreIllegals: true,
        }).value}</code></pre>`
      } catch {
        // 高亮失败时退回转义输出
      }
    }
    return `<pre><code class="hljs">${md.utils.escapeHtml(code)}</code></pre>`
  },
})

function renderMd(text: string): string {
  return md.render(text)
}

function reasoningLive(text: string): string {
  const tail = text.replace(/\s+/g, ' ').trimEnd()
  return tail.length > 48 ? `…${tail.slice(-48)}` : tail
}

onMounted(() => {
  void ai.init()
})
</script>

<template>
  <div class="ai-panel">
    <div class="ai-header">
      <span class="ai-header-title" :title="ai.activeChat.value?.title ?? ''">
        {{ ai.activeChat.value?.title || t('reader.aiAssistant') }}
      </span>
      <el-button text circle size="small" :title="t('reader.aiNewChat')" @click="ai.newChat">
        <el-icon><Plus /></el-icon>
      </el-button>
      <el-button
        text
        circle
        size="small"
        :title="ai.historyOpen.value ? t('reader.aiBackToChat') : t('reader.aiHistory')"
        @click="ai.historyOpen.value = !ai.historyOpen.value"
      >
        <el-icon><ChatDotRound v-if="ai.historyOpen.value" /><Clock v-else /></el-icon>
      </el-button>
    </div>

    <template v-if="!ai.historyOpen.value">
      <div v-if="ai.messages.value.length" :ref="setListEl" class="ai-list">
        <div v-if="ai.hasMoreBefore.value" class="ai-load-more" @click="ai.loadOlder">
          {{ t('reader.aiLoadMore') }}
        </div>
        <template v-for="m in ai.visibleMessages.value" :key="m.id">
          <div
            v-if="m.role !== 'tool' && (m.content || m.reasoning || m.images?.length)"
            :data-idx="m.id"
            :class="['ai-msg', m.role]"
          >
            <img
              v-for="(img, j) in m.images"
              :key="j"
              :src="ai.chatImageUrl(img)"
              class="ai-img"
              alt=""
            />
            <details v-if="m.reasoning" class="ai-reasoning">
              <summary>
                <span>{{ t('reader.aiThinkingLabel') }}</span>
                <span v-if="ai.streamingId.value === m.id" class="ai-reasoning-live">{{
                  reasoningLive(m.reasoning)
                }}</span>
              </summary>
              <div class="ai-reasoning-text">{{ m.reasoning }}</div>
            </details>
            <div v-if="m.content && m.role === 'assistant'" class="ai-md" v-html="renderMd(m.content)" />
            <span v-else-if="m.content">{{ m.content }}</span>
          </div>
        </template>
        <div v-if="ai.toolStatus.value" class="ai-tool-status">
          <el-icon class="is-loading"><Loading /></el-icon>
          <span>{{ t('reader.aiToolRunning', { tool: ai.toolStatus.value }) }}</span>
        </div>
        <div v-if="ai.compacting.value" class="ai-tool-status">
          <el-icon class="is-loading"><Loading /></el-icon>
          <span>{{ t('ai.contextCompacting') }}</span>
        </div>
        <div v-else-if="ai.compactNotice.value" class="ai-tool-status ai-compact-done">
          <span>{{ ai.compactNotice.value }}</span>
        </div>
        <div v-if="ai.hasMoreAfter.value" class="ai-load-more" @click="ai.loadNewer">
          {{ t('reader.aiLoadMore') }}
        </div>
      </div>
      <div v-else class="ai-empty">
        <el-icon :size="22"><ChatDotRound /></el-icon>
        <span>{{ t('reader.aiEmpty') }}</span>
      </div>
    </template>

    <div v-else class="ai-history">
      <div v-if="ai.chats.value.length" class="ai-history-list">
        <div
          v-for="c in ai.chats.value"
          :key="c.id"
          :class="['ai-history-item', { active: c.id === ai.activeChatId.value }]"
          @click="ai.switchChat(c.id)"
        >
          <el-input
            v-if="ai.editingId.value === c.id"
            v-model="ai.editingTitle.value"
            size="small"
            @click.stop
            @keydown.enter.prevent="ai.confirmRename"
            @keydown.escape.prevent="ai.cancelRename"
            @blur="ai.confirmRename"
          />
          <template v-else>
            <div class="ai-history-top">
              <span class="ai-history-title">{{ c.title || t('reader.aiUntitled') }}</span>
              <span v-if="c.id === ai.activeChatId.value" class="ai-history-current">
                {{ t('reader.aiCurrent') }}
              </span>
            </div>
            <div class="ai-history-meta">
              <span>{{ ai.formatTime(c.updated_at) }}</span>
              <span class="ai-history-actions">
                <el-icon :title="t('reader.aiRename')" @click.stop="ai.startRename(c)">
                  <EditPen />
                </el-icon>
                <el-popconfirm
                  :title="t('reader.aiDeleteConfirm')"
                  :width="170"
                  @confirm="ai.removeChat(c.id)"
                >
                  <template #reference>
                    <el-icon class="ai-history-del" :title="t('reader.aiDeleteConv')" @click.stop>
                      <Delete />
                    </el-icon>
                  </template>
                </el-popconfirm>
              </span>
            </div>
          </template>
        </div>
      </div>
      <div v-else class="ai-history-empty">{{ t('reader.aiHistoryEmpty') }}</div>
    </div>

    <div v-if="!ai.historyOpen.value" class="ai-footer">
      <div v-if="ai.pending.value.length" class="ai-pending">
        <div v-for="(p, i) in ai.pending.value" :key="i" class="ai-pending-item">
          <img v-if="p.kind === 'image'" :src="p.value" class="ai-pending-img" alt="" />
          <span v-else class="ai-pending-text">{{ p.value }}</span>
          <el-icon class="ai-pending-del" @click="ai.removePending(i)"><Close /></el-icon>
        </div>
      </div>
      <el-input
        v-model="ai.input.value"
        type="textarea"
        :rows="3"
        resize="none"
        :placeholder="t('reader.aiPlaceholder')"
        @keydown.enter.exact.prevent="ai.send(!!props.visionCapture)"
      />
      <div class="ai-toolbar">
        <el-button
          v-if="(props.visionCapture ? ai.canVision.value : true) && ai.modelKey.value"
          size="small"
          circle
          class="ai-tool-btn"
          :disabled="ai.sending.value || ai.streaming.value"
          @click="ai.onCapture(props.capture, props.visionCapture)"
        >
          <el-icon><Camera /></el-icon>
        </el-button>
        <el-select
          v-model="ai.modelKey.value"
          size="small"
          class="ai-model-select"
          :placeholder="t('reader.aiSelectModel')"
        >
          <el-option v-for="o in ai.modelOptions.value" :key="o.key" :label="o.label" :value="o.key" />
        </el-select>
        <el-select
          v-model="ai.thinking.value"
          size="small"
          class="ai-thinking-select"
          :title="t('reader.aiThinkingTitle')"
        >
          <el-option :label="t('reader.aiThinkingDefault')" value="default" />
          <el-option :label="t('reader.aiThinkingOff')" value="none" />
          <el-option :label="t('reader.aiThinkingLow')" value="low" />
          <el-option :label="t('reader.aiThinkingHigh')" value="high" />
          <el-option :label="t('reader.aiThinkingMax')" value="max" />
        </el-select>
        <el-button
          v-if="ai.streaming.value"
          size="small"
          circle
          class="ai-send"
          :title="t('reader.aiStop')"
          @click="ai.stopStreaming"
        >
          <el-icon><VideoPause /></el-icon>
        </el-button>
        <el-button
          v-else
          type="primary"
          size="small"
          circle
          class="ai-send"
          :disabled="(!ai.pending.value.length && !ai.input.value.trim()) || ai.sending.value || ai.streaming.value"
          @click="ai.send(!!props.visionCapture)"
        >
          <el-icon><Promotion /></el-icon>
        </el-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.ai-header {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 8px 6px 12px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.ai-header-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ai-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-anchor: none;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.ai-load-more {
  align-self: center;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--el-color-primary);
  cursor: pointer;
  user-select: none;
  padding: 2px 8px;
}
.ai-load-more:hover {
  color: var(--el-color-primary-light-3);
}
.ai-tool-status {
  align-self: flex-start;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 4px 10px;
  border-radius: 8px;
  background: var(--el-fill-color-light);
}

.ai-compact-done {
  color: var(--el-color-success);
}

.ai-msg {
  max-width: 85%;
  padding: 6px 10px;
  border-radius: 8px;
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
.ai-msg.user {
  align-self: flex-end;
  background: var(--el-color-primary-light-9);
  border: 1px solid var(--el-color-primary-light-8);
}
.ai-msg.assistant {
  align-self: flex-start;
  background: var(--el-fill-color-light);
}
.ai-img {
  max-width: 100%;
  border-radius: 6px;
  display: block;
}
.ai-empty {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  text-align: center;
  padding: 16px;
}
.ai-history {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 8px;
}
.ai-history-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.ai-history-empty {
  padding: 32px 0;
  text-align: center;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.ai-history-item {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.ai-history-item:hover {
  background: var(--el-fill-color-light);
}
.ai-history-item.active {
  border-color: var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
}
.ai-history-top {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.ai-history-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ai-history-current {
  flex-shrink: 0;
  font-size: 11px;
  line-height: 16px;
  color: var(--el-color-primary);
  border: 1px solid var(--el-color-primary-light-7);
  border-radius: 4px;
  padding: 0 4px;
}
.ai-history-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.ai-history-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--el-text-color-secondary);
}
.ai-history-actions .el-icon {
  cursor: pointer;
}
.ai-history-actions .el-icon:hover {
  color: var(--el-color-primary);
}
.ai-history-del:hover {
  color: var(--el-color-danger);
}
.ai-footer {
  flex-shrink: 0;
  border-top: 1px solid var(--el-border-color-lighter);
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.ai-pending {
  max-height: 150px;
  overflow-y: auto;
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: 6px;
  padding: 6px 8px;
  border: 1px solid var(--el-color-primary-light-7);
  border-radius: 6px;
  background: var(--el-color-primary-light-9);
}
.ai-pending-item {
  position: relative;
  width: 104px;
  height: 64px;
  overflow: hidden;
  border: 1px solid var(--el-color-primary-light-8);
  border-radius: 4px;
  background: var(--el-bg-color);
  display: flex;
  align-items: center;
  justify-content: center;
}
.ai-pending-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.ai-pending-text {
  width: 100%;
  height: 100%;
  padding: 4px 6px;
  font-size: 12px;
  color: var(--el-text-color-primary);
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  word-break: break-word;
}
.ai-pending-del {
  position: absolute;
  top: 3px;
  right: 3px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 10px;
  cursor: pointer;
}
.ai-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}
.ai-tool-btn {
  flex-shrink: 0;
  margin: 0;
}
.ai-model-select {
  flex: 1;
  min-width: 110px;
}
.ai-thinking-select {
  flex-shrink: 0;
  width: 76px;
}
.ai-reasoning {
  margin-bottom: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  border-left: 2px solid var(--el-border-color);
  padding-left: 6px;
}
.ai-reasoning summary {
  cursor: pointer;
  user-select: none;
}
.ai-reasoning-live {
  display: inline-block;
  vertical-align: bottom;
  max-width: calc(100% - 5.5em);
  margin-left: 8px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--el-text-color-placeholder);
}
.ai-reasoning-text {
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-word;
  margin-top: 4px;
}
.ai-send {
  flex-shrink: 0;
  margin: 0;
}
</style>

<style>
.ai-md {
  white-space: normal;
  font-size: 13px;
  line-height: 1.55;
}
.ai-md > :first-child {
  margin-top: 0;
}
.ai-md > :last-child {
  margin-bottom: 0;
}
.ai-md p {
  margin: 4px 0;
}
.ai-md h1,
.ai-md h2,
.ai-md h3,
.ai-md h4,
.ai-md h5,
.ai-md h6 {
  margin: 8px 0 4px;
  font-size: 14px;
}
.ai-md ul,
.ai-md ol {
  margin: 4px 0;
  padding-left: 18px;
}
.ai-md li {
  margin: 2px 0;
}
.ai-md code {
  background: var(--el-fill-color);
  padding: 1px 4px;
  border-radius: 4px;
  font-size: 12px;
}
.ai-md pre {
  background: var(--el-fill-color);
  padding: 8px 10px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 6px 0;
}
.ai-md pre code {
  background: transparent;
  padding: 0;
}
.ai-md blockquote {
  margin: 6px 0;
  padding: 2px 10px;
  border-left: 3px solid var(--el-border-color);
  color: var(--el-text-color-secondary);
}
.ai-md table {
  border-collapse: collapse;
  margin: 6px 0;
}
.ai-md th,
.ai-md td {
  border: 1px solid var(--el-border-color-lighter);
  padding: 3px 8px;
  font-size: 12px;
}
.ai-md a {
  color: var(--el-color-primary);
}
html.dark .ai-md .hljs {
  color: #c9d1d9;
}
html.dark .ai-md .hljs-comment,
html.dark .ai-md .hljs-quote {
  color: #8b949e;
}
html.dark .ai-md .hljs-keyword,
html.dark .ai-md .hljs-doctag,
html.dark .ai-md .hljs-template-tag,
html.dark .ai-md .hljs-type {
  color: #ff7b72;
}
html.dark .ai-md .hljs-title,
html.dark .ai-md .hljs-title.function_,
html.dark .ai-md .hljs-title.class_ {
  color: #d2a8ff;
}
html.dark .ai-md .hljs-attr,
html.dark .ai-md .hljs-attribute,
html.dark .ai-md .hljs-literal,
html.dark .ai-md .hljs-meta,
html.dark .ai-md .hljs-number,
html.dark .ai-md .hljs-operator,
html.dark .ai-md .hljs-variable,
html.dark .ai-md .hljs-selector-attr,
html.dark .ai-md .hljs-selector-class,
html.dark .ai-md .hljs-selector-id {
  color: #79c0ff;
}
html.dark .ai-md .hljs-string,
html.dark .ai-md .hljs-regexp {
  color: #a5d6ff;
}
html.dark .ai-md .hljs-built_in,
html.dark .ai-md .hljs-symbol,
html.dark .ai-md .hljs-bullet,
html.dark .ai-md .hljs-link {
  color: #ffa657;
}
html.dark .ai-md .hljs-section,
html.dark .ai-md .hljs-name {
  color: #7ee787;
}
</style>
