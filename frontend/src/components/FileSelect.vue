<template>
  <el-tree-select
    v-model="selectedPath"
    :data="treeData"
    :props="treeProps"
    :load="loadNodes"
    lazy
    check-strictly
    :render-after-expand="false"
    :placeholder="placeholder"
    :disabled="disabled"
    clearable
    style="width: 100%"
  />
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { browseFiles, type FileItem } from '@/api/system'

interface TreeNode {
  value: string
  label: string
  children?: TreeNode[]
  isLeaf?: boolean
}

const { t } = useI18n()

const props = defineProps<{
  modelValue: string
  placeholder?: string
  disabled?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const selectedPath = ref(props.modelValue || '')
const treeData = ref<TreeNode[]>([])
const treeProps = {
  label: 'label',
  children: 'children',
  isLeaf: (data: TreeNode) => data.isLeaf === true,
  disabled: (data: TreeNode) => !data.isLeaf,
}

watch(() => props.modelValue, (newVal) => {
  selectedPath.value = newVal || ''
})

watch(selectedPath, (newVal) => {
  emit('update:modelValue', newVal || '')
})

const loadNodes = async (node: { level: number; data?: TreeNode }, resolve: (data: TreeNode[]) => void) => {
  if (node.level === 0) {
    resolve([
      {
        value: '/',
        label: t('init.rootDirectory'),
        isLeaf: false,
      },
    ])
    return
  }

  const path = node.data?.value === '/' ? '' : (node.data?.value || '')

  try {
    const response = await browseFiles(path)
    const items = response.files
      .sort((a: FileItem, b: FileItem) => {
        if (a.file_type !== b.file_type) return a.file_type === 'directory' ? -1 : 1
        return a.name.localeCompare(b.name)
      })
      .map((file: FileItem) => ({
        value: path ? `${path}/${file.name}` : file.name,
        label: file.name,
        isLeaf: file.file_type !== 'directory',
      }))

    resolve(items)
  } catch {
    resolve([])
  }
}

defineExpose({
  refresh: () => {
    treeData.value = []
  }
})
</script>
