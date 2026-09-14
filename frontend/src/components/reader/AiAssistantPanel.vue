<script setup lang="ts">
import AiChatPanel from '@/components/ai/AiChatPanel.vue'

const props = defineProps<{
  format?: string
  bookId: string
  capture?: () => Promise<string | null>
  pageQuery?: (kind: string, params: Record<string, unknown>) => Promise<string | null>
}>()
</script>

<template>
  <AiChatPanel
    biz-type="ebook"
    :biz-id="props.bookId"
    :capture="props.capture"
    :page-query="props.pageQuery"
    :vision-capture="props.format === 'pdf'"
  />
</template>
