<template>
  <div class="notes-container" @click="hideContextMenu" @contextmenu="hideContextMenu">
    <!-- ===== Mobile View ===== -->
    <div v-show="isMobile" class="mobile-view">
      <div v-if="!currentNotebook" class="mobile-notebook-list">
        <div
          v-for="notebook in notebookStore.notebooks"
          :key="notebook.id"
          class="mobile-notebook-card"
          @click="selectNotebook(notebook)"
        >
          <div class="mobile-notebook-icon">
            <el-icon v-if="!notebook.encrypted || cryptoStore.isUnlocked(notebook.id)" :size="22"><Unlock /></el-icon>
            <el-icon v-else :size="22"><Lock /></el-icon>
          </div>
          <div class="mobile-notebook-info">
            <div class="mobile-notebook-name">{{ notebook.name }}</div>
            <div v-if="notebook.description" class="mobile-notebook-desc">{{ notebook.description }}</div>
          </div>
          <el-icon class="mobile-notebook-arrow"><ArrowRight /></el-icon>
        </div>
        <div v-if="notebookStore.notebooks.length === 0" class="mobile-empty">
          {{ t('notes.noNotebooks') }}
        </div>
      </div>

      <div v-else-if="!noteStore.currentNote && !mobileSearchActive" class="mobile-note-view">
        <div class="mobile-note-toolbar">
          <div class="mobile-note-back" @click="mobileCurrentFolderPath ? (mobileCurrentFolderPath = '') : (currentNotebook = null)">
            <el-icon><ArrowLeft /></el-icon>
            <span>{{ mobileCurrentFolderPath ? mobileFolderLabel : currentNotebook.name }}</span>
          </div>
          <el-icon class="mobile-search-btn" @click="mobileSearchActive = true"><Search /></el-icon>
        </div>
        <div class="mobile-tree-list">
          <div
            v-for="node in mobileTreeData"
            :key="node.id"
            class="mobile-tree-item"
            :class="{ 'mobile-tree-item-attachment': node.isAttachment, 'mobile-tree-item-folder': node.isFolder }"
            @click="handleMobileNodeClick(node)"
          >
            <el-icon v-if="node.isFolder" :size="18"><Folder /></el-icon>
            <el-icon v-else-if="node.isNote" :size="18"><Document /></el-icon>
            <el-icon v-else :size="18"><Document /></el-icon>
            <span>{{ node.label }}</span>
            <el-icon v-if="node.isFolder" class="mobile-tree-item-arrow"><ArrowRight /></el-icon>
          </div>
          <div v-if="mobileTreeData.length === 0" class="mobile-empty">
            {{ t('notes.selectNoteHint') }}
          </div>
        </div>
      </div>

      <div v-else-if="mobileSearchActive && !noteStore.currentNote" class="mobile-note-view">
        <div class="mobile-note-toolbar">
          <div class="mobile-note-back" @click="clearMobileSearch">
            <el-icon><ArrowLeft /></el-icon>
            <span>{{ currentNotebook.name }}</span>
          </div>
        </div>
        <div class="mobile-search-input-wrap">
          <el-input
            v-model="mobileSearchKeyword"
            :placeholder="t('notes.searchTreePlaceholder')"
            clearable
            size="small"
            @keyup.enter="executeMobileSearch"
            @clear="executeMobileSearch"
          />
        </div>
        <div class="mobile-tree-list">
          <div v-if="mobileSearchLoading" style="text-align: center; padding: 20px;">
            <el-icon class="is-loading" :size="24"><Loading /></el-icon>
          </div>
          <template v-else-if="searchTreeData.length > 0">
            <div v-for="nb in searchTreeData" :key="nb.id">
              <div class="search-tree-notebook" @click="toggleSearchGroup(nb.id)">
                <el-icon :size="16"><Notebook /></el-icon>
                <span>{{ nb.label }}</span>
                <el-icon class="search-tree-arrow" :class="{ 'is-expanded': expandedSearchGroups.has(nb.id) }"><ArrowRight /></el-icon>
              </div>
              <div v-if="expandedSearchGroups.has(nb.id)" class="search-tree-children">
                <div v-for="note in nb.children" :key="note.id" class="search-tree-note">
                  <div class="search-tree-note-title" @click="openSearchResultItem(nb.notebookId, note.notePath, nb.encrypted, undefined, note.label)">
                    <el-icon :size="14"><Document /></el-icon>
                    <span>{{ note.label }}</span>
                  </div>
                  <div v-for="m in note.matchItems" :key="m.id" class="search-tree-match" @click="openSearchResultItem(nb.notebookId, note.notePath, nb.encrypted, m.lineNumber, note.label)">
                    <span class="search-match-line">{{ m.lineNumber + 1 }}:</span>
                    <span class="search-match-snippet" v-html="renderMatchContent(m.content)"></span>
                  </div>
                </div>
              </div>
            </div>
          </template>
          <div v-else-if="mobileSearchKeyword" class="mobile-empty">
            {{ t('notes.noSearchResults') }}
          </div>
          <div v-else class="mobile-empty">
            {{ t('notes.searchTreePlaceholder') }}
          </div>
        </div>
      </div>

      <div v-else class="mobile-note-view">
        <div class="mobile-note-toolbar">
          <div class="mobile-note-back" @click="handleMobileBack">
            <el-icon><ArrowLeft /></el-icon>
            <span>{{ noteTitle }}</span>
          </div>
          <el-button size="small" type="primary" :loading="noteStore.currentNote?.isSaving" @click="handleSaveNote">
            {{ t('notes.save') }}
          </el-button>
        </div>
        <div v-if="mobilePreviewMode" class="mobile-preview">
          <div class="preview-container" v-html="renderedContent"></div>
        </div>
        <div v-else class="mobile-editor">
          <textarea
            class="mobile-textarea"
            :value="noteStore.currentNote?.content"
            @input="noteStore.updateContent(($event.target as HTMLTextAreaElement).value)"
          />
        </div>
        <div class="mobile-fab" @click="mobilePreviewMode = !mobilePreviewMode">
          <el-icon :size="22"><component :is="mobilePreviewMode ? Edit : View" /></el-icon>
        </div>
      </div>
    </div>

    <!-- ===== Desktop View ===== -->
    <div v-show="!isMobile" class="desktop-view">
      <div class="notebook-panel">
        <div class="tree-search">
          <el-input
            v-model="searchKeyword"
            :placeholder="t('notes.searchTreePlaceholder')"
            :prefix-icon="Search"
            clearable
            size="small"
            @keyup.enter="executeSearch"
            @clear="clearSearch"
          />
        </div>
        <div v-show="isSearchActive" class="tree-scroll search-tree-scroll">
          <div v-if="searchLoading" style="text-align: center; padding: 20px;">
            <el-icon class="is-loading" :size="24"><Loading /></el-icon>
          </div>
          <template v-else-if="searchTreeData.length > 0">
            <div v-for="nb in searchTreeData" :key="nb.id" class="search-tree-group">
              <div class="search-tree-notebook" @click="toggleSearchGroup(nb.id)">
                <el-icon :size="16"><Notebook /></el-icon>
                <span>{{ nb.label }}</span>
                <el-icon class="search-tree-arrow" :class="{ 'is-expanded': expandedSearchGroups.has(nb.id) }"><ArrowRight /></el-icon>
              </div>
              <div v-if="expandedSearchGroups.has(nb.id)" class="search-tree-children">
                <div v-for="note in nb.children" :key="note.id" class="search-tree-note">
                  <div class="search-tree-note-title" @click="openSearchResultItem(nb.notebookId, note.notePath, nb.encrypted, undefined, note.label)">
                    <el-icon :size="14"><Document /></el-icon>
                    <span>{{ note.label }}</span>
                  </div>
                  <div v-for="m in note.matchItems" :key="m.id" class="search-tree-match" @click="openSearchResultItem(nb.notebookId, note.notePath, nb.encrypted, m.lineNumber, note.label)">
                    <span class="search-match-line">{{ m.lineNumber + 1 }}:</span>
                    <span class="search-match-snippet" v-html="renderMatchContent(m.content)"></span>
                  </div>
                </div>
              </div>
            </div>
          </template>
          <div v-else class="search-empty">
            {{ t('notes.noSearchResults') }}
          </div>
        </div>
        <div v-show="!isSearchActive" class="tree-scroll">
        <el-tree
          ref="treeRef"
          :data="treeData"
          :props="treeProps"
          :load="loadNotebookNode"
          lazy
          highlight-current
          node-key="id"
          :default-expanded-keys="defaultExpandedKeys"
          :expand-on-click-node="false"
          style="width: fit-content; min-width: 100%"
          @contextmenu.prevent
          @node-click="handleNodeClick"
          @node-contextmenu="handleNodeContextMenu"
        >
          <template #default="{ data }">
            <span class="tree-node">
              <el-icon v-if="data.isRoot"><Notebook /></el-icon>
              <el-icon v-else-if="data.isAttachmentFolder" class="tree-node-folder attachment-folder"><Paperclip /></el-icon>
              <el-icon v-else-if="data.isFolder" class="tree-node-folder"><Folder /></el-icon>
              <el-icon v-else-if="data.isAttachment" class="tree-node-attachment"><Paperclip /></el-icon>
              <el-icon v-else-if="data.isNote" class="tree-node-note"><Document /></el-icon>
              <el-icon v-else-if="!notebookStore.getNotebookById(data.id)?.encrypted || cryptoStore.isUnlocked(data.id)" class="tree-node-unlocked"><Unlock /></el-icon>
              <el-icon v-else class="tree-node-locked"><Lock /></el-icon>
              <span :class="{ 'attachment-folder-label': data.isAttachmentFolder }">{{ data.label }}</span>
            </span>
          </template>
        </el-tree>
        </div>
      </div>

      <div v-if="currentNotebook" class="note-panel">
        <div v-if="noteStore.currentNote" class="editor-container">
          <div class="editor-header">
            <span class="note-title">{{ noteTitle }}</span>
            <span v-if="noteStore.currentNote.isSaving" class="save-status">{{ t('notes.saving') }}</span>
            <span v-else-if="noteStore.currentNote.isDirty" class="save-status unsaved">{{ t('notes.unsaved') }}</span>
          </div>
          <div class="editor-toolbar">
            <el-tooltip :content="t('notes.toolbar.bold')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('bold')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M15.6 10.79c.97-.67 1.65-1.77 1.65-2.79 0-2.26-1.75-4-4-4H7v14h7.04c2.09 0 3.71-1.7 3.71-3.79 0-1.52-.86-2.82-2.15-3.42zM10 6.5h3c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5H10v-3zm3.5 9H10v-3h3.5c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.italic')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('italic')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M10 4v3h2.21l-3.42 8H6v3h8v-3h-2.21l3.42-8H18V4z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.link')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('link')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1H15v1.9h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z"/></svg>
              </span>
            </el-tooltip>
            <div class="toolbar-divider"></div>
            <el-tooltip :content="t('notes.toolbar.unorderedList')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('unorderedList')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M4 10.5c-.83 0-1.5.67-1.5 1.5s.67 1.5 1.5 1.5 1.5-.67 1.5-1.5-.67-1.5-1.5-1.5zm0-6c-.83 0-1.5.67-1.5 1.5S3.17 7.5 4 7.5 5.5 6.83 5.5 6 4.83 4.5 4 4.5zm0 12c-.83 0-1.5.68-1.5 1.5s.68 1.5 1.5 1.5 1.5-.68 1.5-1.5-.68-1.5-1.5-1.5zM7 19h14v-2H7v2zm0-6h14v-2H7v2zm0-8v2h14V5H7z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.orderedList')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('orderedList')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M2 17h2v.5H3v1h1v.5H2v1h3v-4H2v1zm1-9h1V4H2v1h1v3zm-1 3h1.8L2 13.1v.9h3v-1H3.2L5 10.9V10H2v1zm5-6v2h14V5H7zm0 14h14v-2H7v2zm0-6h14v-2H7v2z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.heading')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('heading')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M11 7h2v10h-2zm4 6h2v4h-2zm-8-4h2v8H7zm8-2h2v2h-2z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.horizontalRule')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('horizontalRule')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M2 11h20v2H2z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.table')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="insertFormat('table')">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M3 3v18h18V3H3zm8 14H5v-4h6v4zm0-6H5V7h6v4zm8 6h-6v-4h6v4zm0-6h-6V7h6v4z"/></svg>
              </span>
            </el-tooltip>
            <div class="toolbar-divider"></div>
            <el-tooltip :content="t('notes.toolbar.imageUpload')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="triggerImageUpload">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z"/></svg>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('notes.toolbar.fileUpload')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="triggerFileUpload">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 14h-3v3h-2v-3H8v-2h3v-3h2v3h3v2zm-3-7V3.5L18.5 9H13z"/></svg>
              </span>
            </el-tooltip>
            <div class="toolbar-divider"></div>
            <el-tooltip :content="t('notes.save')" placement="top" :show-after="500">
              <span class="toolbar-btn" @click="handleSaveNote">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M17 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V7l-4-4zm-5 16c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3zm3-10H5V5h10v4z"/></svg>
              </span>
            </el-tooltip>
          </div>
          <div class="editor-wrapper">
            <div class="editor-pane">
              <div ref="editorContainer" class="editor-container"></div>
            </div>
            <div class="editor-divider"></div>
            <div class="preview-pane">
              <div class="preview-container" v-html="renderedContent"></div>
            </div>
          </div>
        </div>
        <div v-else class="note-empty">
          <el-icon :size="48" color="var(--el-text-color-secondary)"><Document /></el-icon>
          <p>{{ t('notes.selectNoteHint') }}</p>
        </div>
      </div>

      <div v-else class="note-empty">
        <el-icon :size="48" color="var(--el-text-color-secondary)"><Document /></el-icon>
        <p>{{ t('notes.selectNoteHint') }}</p>
      </div>
    </div>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.visible"
      class="context-menu"
      :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
      @click.stop
      @contextmenu.stop
    >
      <template v-if="contextMenu.isRoot">
        <div class="context-menu-item" @click="handleCreateNotebook">
          <el-icon><Plus /></el-icon>
          {{ t('notes.create') }}
        </div>
        <div class="context-menu-item" @click="handleImportNotebook">
          <el-icon><Upload /></el-icon>
          {{ t('notes.import') }}
        </div>
      </template>
      <template v-else-if="contextMenu.isAttachmentFolder">
        <div class="context-menu-item" @click="handleAttachmentCleanup">
          <el-icon><Delete /></el-icon>
          {{ t('notes.attachmentCleanup') }}
        </div>
      </template>
      <template v-else-if="contextMenu.isFolder">
        <div class="context-menu-item" @click="handleCreateNote">
          <el-icon><DocumentAdd /></el-icon>
          {{ t('notes.createNote') }}
        </div>
        <div class="context-menu-item" @click="handleCreateFolder">
          <el-icon><FolderAdd /></el-icon>
          {{ t('notes.createFolder') }}
        </div>
        <div class="context-menu-divider" />
        <div class="context-menu-item" @click="handleRenameNode">
          <el-icon><Edit /></el-icon>
          {{ t('common.rename') }}
        </div>
        <div class="context-menu-item" @click="handleMoveTo">
          <el-icon><Rank /></el-icon>
          {{ t('notes.moveTo') }}
        </div>
        <div class="context-menu-item context-menu-danger" @click="handleDeleteNode">
          <el-icon><Delete /></el-icon>
          {{ t('notes.delete') }}
        </div>
      </template>
      <template v-else-if="contextMenu.isNote">
        <div class="context-menu-item" @click="handleRenameNode">
          <el-icon><Edit /></el-icon>
          {{ t('common.rename') }}
        </div>
        <div class="context-menu-item" @click="handleMoveTo">
          <el-icon><Rank /></el-icon>
          {{ t('notes.moveTo') }}
        </div>
        <div class="context-menu-item context-menu-danger" @click="handleDeleteNode">
          <el-icon><Delete /></el-icon>
          {{ t('notes.delete') }}
        </div>
      </template>
      <template v-else-if="contextMenu.isAttachment">
        <div class="context-menu-item" @click="handleDownloadAttachment">
          <el-icon><Download /></el-icon>
          {{ t('notes.downloadAttachment') }}
        </div>
        <div class="context-menu-item context-menu-danger" @click="handleDeleteAttachment">
          <el-icon><Delete /></el-icon>
          {{ t('notes.deleteAttachment') }}
        </div>
      </template>
      <template v-else>
        <div class="context-menu-item" @click="handleEditNotebook">
          <el-icon><Edit /></el-icon>
          {{ t('notes.edit') }}
        </div>
        <div v-if="notebookStore.getNotebookById(contextMenu.notebookId)?.encrypted && cryptoStore.isUnlocked(contextMenu.notebookId)" class="context-menu-item" @click="handleCloseNotebook">
          <el-icon><Lock /></el-icon>
          {{ t('notes.closeNotebook') }}
        </div>
        <div class="context-menu-item context-menu-danger" @click="handleDeleteNotebookCtx">
          <el-icon><Delete /></el-icon>
          {{ t('notes.delete') }}
        </div>
        <template v-if="!notebookStore.getNotebookById(contextMenu.notebookId)?.encrypted || cryptoStore.isUnlocked(contextMenu.notebookId)">
          <div class="context-menu-divider" />
          <div class="context-menu-item" @click="handleCreateNote">
            <el-icon><DocumentAdd /></el-icon>
            {{ t('notes.createNote') }}
          </div>
          <div class="context-menu-item" @click="handleCreateFolder">
            <el-icon><FolderAdd /></el-icon>
            {{ t('notes.createFolder') }}
          </div>
        </template>
      </template>
    </div>

    <!-- Create Notebook Dialog -->
    <el-dialog v-model="createNotebookVisible" :title="t('notes.createNotebook')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form ref="createNotebookFormRef" :model="createNotebookForm" :rules="createNotebookRules" label-position="top" @submit.prevent>
        <el-form-item :label="t('notes.notebookName')" prop="name">
          <el-input v-model="createNotebookForm.name" :placeholder="t('notes.notebookNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('notes.notebookDescription')">
          <el-input v-model="createNotebookForm.description" type="textarea" :rows="3" :placeholder="t('notes.notebookDescriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('notes.storagePath')" prop="path">
          <FolderSelect v-model="createNotebookForm.path" :placeholder="t('notes.storagePathPlaceholder')" />
        </el-form-item>
        <el-form-item>
          <div class="encrypt-switch-row">
            <el-switch v-model="createNotebookForm.encrypted" />
            <span class="encrypt-switch-label">{{ t('notes.enableEncryption') }}</span>
          </div>
        </el-form-item>
        <el-form-item v-if="createNotebookForm.encrypted" :label="t('notes.encryptPassword')" prop="password">
          <el-input v-model="createNotebookForm.password" type="password" show-password :placeholder="t('notes.encryptPasswordPlaceholder')" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createNotebookVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="createNotebookLoading" @click="submitCreateNotebook">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Import Notebook Dialog -->
    <el-dialog v-model="importNotebookVisible" :title="t('notes.importNotebook')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form ref="importNotebookFormRef" :model="importNotebookForm" :rules="importNotebookRules" label-position="top" @submit.prevent>
        <el-form-item :label="t('notes.notebookName')" prop="name">
          <el-input v-model="importNotebookForm.name" :placeholder="t('notes.notebookNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('notes.notebookDescription')">
          <el-input v-model="importNotebookForm.description" type="textarea" :rows="3" :placeholder="t('notes.notebookDescriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('notes.storagePath')" prop="path">
          <FolderSelect v-model="importNotebookForm.path" :placeholder="t('notes.storagePathPlaceholder')" />
        </el-form-item>
        <el-form-item>
          <div class="encrypt-switch-row">
            <el-switch v-model="importNotebookForm.encrypted" />
            <span class="encrypt-switch-label">{{ t('notes.enableEncryption') }}</span>
          </div>
        </el-form-item>
        <el-form-item v-if="importNotebookForm.encrypted" :label="t('notes.encryptPassword')" prop="password">
          <el-input v-model="importNotebookForm.password" type="password" show-password :placeholder="t('notes.encryptPasswordPlaceholder')" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="importNotebookVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="importNotebookLoading" @click="submitImportNotebook">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Edit Notebook Dialog -->
    <el-dialog v-model="editNotebookVisible" :title="t('notes.editNotebook')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form ref="editNotebookFormRef" :model="editNotebookForm" :rules="editNotebookRules" label-position="top" @submit.prevent>
        <el-form-item :label="t('notes.notebookName')" prop="name">
          <el-input v-model="editNotebookForm.name" :placeholder="t('notes.notebookNamePlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('notes.notebookDescription')">
          <el-input v-model="editNotebookForm.description" type="textarea" :rows="3" :placeholder="t('notes.notebookDescriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('notes.storagePath')">
          <FolderSelect v-model="editNotebookForm.path" :placeholder="t('notes.storagePathPlaceholder')" disabled />
        </el-form-item>
        <el-form-item>
          <div class="encrypt-switch-row">
            <el-switch v-model="editNotebookForm.encrypted" disabled />
            <span class="encrypt-switch-label">{{ t('notes.enableEncryption') }}</span>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editNotebookVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="editNotebookLoading" @click="submitEditNotebook">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Unlock Notebook Dialog -->
    <el-dialog v-model="unlockDialogVisible" :title="t('notes.unlockNotebook')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item :label="t('notes.encryptPassword')">
          <el-input v-model="unlockPassword" type="password" show-password :placeholder="t('notes.encryptPasswordPlaceholder')" @keyup.enter="submitUnlock" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="unlockDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="unlockLoading" @click="submitUnlock">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Note Dialog -->
    <el-dialog v-model="noteDialogVisible" :title="t('notes.createNote')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item>
          <el-input v-model="noteFormName" :placeholder="t('notes.noteNamePlaceholder')" @keyup.enter="submitNoteForm" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="noteDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="noteFormLoading" @click="submitNoteForm">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Rename Dialog -->
    <el-dialog v-model="renameDialogVisible" :title="t('common.rename')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item>
          <el-input v-model="renameFormName" :placeholder="t('notes.noteNamePlaceholder')" @keyup.enter="submitRename" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="renameDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="renameFormLoading" @click="submitRename">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Move To Dialog -->
    <el-dialog v-model="moveToDialogVisible" :title="t('notes.moveToDialogTitle')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item>
          <el-tree
            :data="moveToFolderTree"
            :props="{ children: 'children', label: 'label' }"
            highlight-current
            node-key="path"
            default-expand-all
            :expand-on-click-node="false"
            class="move-to-tree"
            @node-click="handleMoveToFolderClick"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="moveToDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="moveToLoading" @click="submitMoveTo">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- Folder Dialog -->
    <el-dialog v-model="folderDialogVisible" :title="t('notes.createFolder')" :width="isMobile ? '90%' : '420px'" destroy-on-close>
      <el-form label-position="top" @submit.prevent>
        <el-form-item>
          <el-input v-model="folderFormName" :placeholder="t('notes.folderNamePlaceholder')" @keyup.enter="submitFolderForm" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="folderDialogVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" :loading="folderFormLoading" @click="submitFolderForm">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="cleanupDialogVisible" :title="t('notes.attachmentCleanup')" :width="isMobile ? '90%' : '480px'" :close-on-click-modal="false" :close-on-press-escape="false" :show-close="cleanupPhase === 'done'">
      <div v-if="cleanupPhase === 'checking'" style="text-align: center; padding: 10px 0;">
        <p>{{ t('notes.cleanupChecking') }} ({{ cleanupChecked }}/{{ cleanupTotal }})</p>
        <el-progress :percentage="cleanupTotal > 0 ? Math.round(cleanupChecked / cleanupTotal * 100) : 0" :stroke-width="18" :text-inside="true" />
      </div>
      <div v-else-if="cleanupPhase === 'confirm'">
        <p style="text-align: center;">{{ t('notes.cleanupConfirmCount', { count: cleanupUnreferenced.length }) }}</p>
        <div style="max-height: 240px; overflow-y: auto; margin-top: 8px; border: 1px solid var(--el-border-color-lighter); border-radius: 4px; padding: 8px;">
          <div v-for="name in cleanupDisplayNames.slice(0, 100)" :key="name" style="padding: 2px 0; font-size: 13px; color: var(--el-text-color-regular); word-break: break-all;">
            {{ name }}
          </div>
          <div v-if="cleanupUnreferenced.length > 100" style="padding: 4px 0; font-size: 13px; color: var(--el-text-color-secondary);">
            {{ t('notes.cleanupMoreItems', { count: cleanupUnreferenced.length - 100 }) }}
          </div>
        </div>
      </div>
      <div v-else-if="cleanupPhase === 'cleaning'" style="text-align: center; padding: 10px 0;">
        <p>{{ t('notes.cleanupCleaning') }}</p>
      </div>
      <div v-else-if="cleanupPhase === 'done'" style="text-align: center; padding: 10px 0;">
        <p v-if="cleanupUnreferenced.length > 0">{{ t('notes.attachmentCleanupSuccess') }}</p>
        <p v-else>{{ t('notes.cleanupNoOrphan') }}</p>
      </div>
      <template #footer>
        <template v-if="cleanupPhase === 'confirm'">
          <el-button @click="cleanupDialogVisible = false">{{ t('common.cancel') }}</el-button>
          <el-button type="primary" @click="executeCleanup">{{ t('common.confirm') }}</el-button>
        </template>
        <template v-else-if="cleanupPhase === 'done'">
          <el-button type="primary" @click="cleanupDialogVisible = false">{{ t('common.confirm') }}</el-button>
        </template>
      </template>
    </el-dialog>
    <input ref="imageInputRef" type="file" accept="image/*" style="display:none" @change="handleImageUpload" />
    <input ref="fileInputRef" type="file" style="display:none" @change="handleFileUpload" />
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted, onUnmounted, nextTick, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from '@/utils/message'
import { ElMessageBox } from 'element-plus'
import {
  Search,
  Plus,
  Edit,
  Delete,
  Lock,
  Unlock,
  ArrowLeft,
  ArrowRight,
  Upload,
  Download,
  Folder,
  FolderAdd,
  Document,
  Paperclip,
  Notebook,
  DocumentAdd,
  Loading,
  Rank,
  View,
} from '@element-plus/icons-vue'
import type { FormInstance, FormRules } from 'element-plus'
import type { ElTree } from 'element-plus'
import FolderSelect from '@/components/FolderSelect.vue'
import { useThemeStore } from '@/stores/theme'
import { useNotebookStore } from '@/stores/notebook'
import { useNoteStore } from '@/stores/note'
import { useCryptoStore } from '@/stores/crypto'
import {
  type NotebookItem,
  type FileTreeNode,
} from '@/api/system'
import { saveNote as apiSaveNote, readNote as readNoteApi, getAttachmentUrl, searchNotes, createNotebookFolder } from '@/api/system'
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightActiveLine, drawSelection, rectangularSelection, highlightSpecialChars } from '@codemirror/view'
import { EditorState, Compartment } from '@codemirror/state'
import { markdown } from '@codemirror/lang-markdown'
import { languages } from '@codemirror/language-data'
import { oneDark } from '@codemirror/theme-one-dark'
import { defaultKeymap, indentWithTab, history, historyKeymap } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, foldGutter, indentOnInput } from '@codemirror/language'
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search'
import { closeBrackets, closeBracketsKeymap, autocompletion, completionKeymap } from '@codemirror/autocomplete'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

const { t } = useI18n()
const themeStore = useThemeStore()
const notebookStore = useNotebookStore()
const noteStore = useNoteStore()
const cryptoStore = useCryptoStore()

const renderer = new marked.Renderer()
const originalLink = renderer.link
renderer.link = function (this: any, token: any) {
  const html = originalLink.call(this, token)
  return html.replace('<a ', '<a target="_blank" rel="noopener noreferrer" ')
}
marked.setOptions({ gfm: true, breaks: true, renderer })

const editorContainer = ref<HTMLElement | null>(null)
const editorInstance = shallowRef<EditorView | null>(null)
const themeCompartment = new Compartment()

interface TreeNodeData {
  id: string
  label: string
  isRoot?: boolean
  isNotebook?: boolean
  isFolder?: boolean
  isNote?: boolean
  isAttachment?: boolean
  isAttachmentFolder?: boolean
  notebookId?: string
  path?: string
  isLeaf?: boolean
  children?: TreeNodeData[]
}

const isMobile = ref(false)

const mobileCurrentFolderPath = ref('')

const mobileTreeData = computed<TreeNodeData[]>(() => {
  if (!currentNotebook.value) return []
  const nb = currentNotebook.value
  const fileTree = notebookStore.fileTreeMap.get(nb.id)
  if (!fileTree) return []
  let nodes: FileTreeNode[] = fileTree
  if (mobileCurrentFolderPath.value) {
    const found = findChildrenByPath(fileTree, mobileCurrentFolderPath.value)
    if (!found) return []
    nodes = found
  }
  return convertTree(nodes, nb.id).filter(n => !n.isAttachmentFolder)
})

const mobileFolderLabel = computed(() => {
  if (!mobileCurrentFolderPath.value || !currentNotebook.value) return ''
  const fileTree = notebookStore.fileTreeMap.get(currentNotebook.value.id)
  if (!fileTree) return mobileCurrentFolderPath.value.split('/').pop() || ''
  const parentPath = mobileCurrentFolderPath.value.includes('/') ? mobileCurrentFolderPath.value.substring(0, mobileCurrentFolderPath.value.lastIndexOf('/')) : ''
  const siblings = parentPath ? findChildrenByPath(fileTree, parentPath) : fileTree
  if (!siblings) return mobileCurrentFolderPath.value.split('/').pop() || ''
  const folderName = mobileCurrentFolderPath.value.split('/').pop() || ''
  const found = siblings.find(n => n.path === mobileCurrentFolderPath.value)
  return found ? found.name : folderName
})

const currentNotebook = ref<NotebookItem | null>(null)
const treeRef = ref<InstanceType<typeof ElTree>>()

const unlockDialogVisible = ref(false)
const unlockPassword = ref('')
const unlockingNotebookId = ref('')

const searchKeyword = ref('')
const searchLoading = ref(false)
const isSearchActive = ref(false)
const expandedSearchGroups = ref<Set<string>>(new Set())

const mobileSearchKeyword = ref('')
const mobileSearchActive = ref(false)
const mobileSearchLoading = ref(false)
const mobilePreviewMode = ref(false)

interface SearchTreeNotebook {
  id: string
  label: string
  notebookId: string
  encrypted: boolean
  children: SearchTreeNote[]
}

interface SearchTreeNote {
  id: string
  label: string
  notePath: string
  matchItems: SearchTreeMatch[]
}

interface SearchTreeMatch {
  id: string
  lineNumber: number
  content: string
}

const searchTreeData = ref<SearchTreeNotebook[]>([])

const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  isRoot: false,
  isFolder: false,
  isAttachmentFolder: false,
  isNote: false,
  isAttachment: false,
  notebookId: '',
  path: '',
  label: '',
})

const createNotebookVisible = ref(false)
const importNotebookVisible = ref(false)
const editNotebookVisible = ref(false)
const noteDialogVisible = ref(false)
const folderDialogVisible = ref(false)
const renameDialogVisible = ref(false)
const createNotebookLoading = ref(false)
const imageInputRef = ref<HTMLInputElement | null>(null)
const fileInputRef = ref<HTMLInputElement | null>(null)
const importNotebookLoading = ref(false)
const editNotebookLoading = ref(false)
const noteFormLoading = ref(false)
const folderFormLoading = ref(false)
const renameFormLoading = ref(false)
const unlockLoading = ref(false)

const noteFormName = ref('')
const noteParentPath = ref('')
const noteNotebookId = ref('')

const folderFormName = ref('')
const folderParentPath = ref('')
const folderNotebookId = ref('')

const renameFormName = ref('')
const renamePath = ref('')
const renameNotebookId = ref('')
const renameIsFolder = ref(false)

const moveToDialogVisible = ref(false)
const moveToLoading = ref(false)
const moveToNotebookId = ref('')
const moveToSourcePath = ref('')
const moveToTargetFolder = ref('')
const moveToSourceIsFolder = ref(false)

const createNotebookFormRef = ref<FormInstance>()
const importNotebookFormRef = ref<FormInstance>()
const editNotebookFormRef = ref<FormInstance>()

const createNotebookForm = reactive({ name: '', description: '', path: '', encrypted: false, password: '' })
const importNotebookForm = reactive({ name: '', description: '', encrypted: false, password: '', path: '' })
const editNotebookForm = reactive({ id: '', name: '', description: '', encrypted: false, path: '' })


const cleanupDialogVisible = ref(false)
const cleanupPhase = ref<'checking' | 'confirm' | 'cleaning' | 'done'>('checking')
const cleanupChecked = ref(0)
const cleanupTotal = ref(0)
const cleanupUnreferenced = ref<string[]>([])
const cleanupDisplayNames = ref<string[]>([])
const cleanupNotebookId = ref('')

const createNotebookRules = computed<FormRules>(() => ({
  name: [{ required: true, message: t('notes.notebookNameRequired'), trigger: 'blur' }],
  path: [{ required: true, message: t('notes.storagePathRequired'), trigger: 'change' }],
  password: [{ required: createNotebookForm.encrypted, message: t('notes.encryptPasswordRequired'), trigger: 'blur' }],
}))

const importNotebookRules = computed<FormRules>(() => ({
  name: [{ required: true, message: t('notes.notebookNameRequired'), trigger: 'blur' }],
  path: [{ required: true, message: t('notes.storagePathRequired'), trigger: 'change' }],
  password: [{ required: importNotebookForm.encrypted, message: t('notes.encryptPasswordRequired'), trigger: 'blur' }],
}))

const editNotebookRules = computed<FormRules>(() => ({
  name: [{ required: true, message: t('notes.notebookNameRequired'), trigger: 'blur' }],
}))

const noteTitle = computed(() => {
  return noteStore.currentNote?.title || ''
})

const renderedContent = ref('')

async function updateRenderedContent() {
  const content = noteStore.currentNote?.content
  if (!content) { renderedContent.value = ''; return }
  let processed = content
  const notebookId = noteStore.currentNote!.notebookId
  let token = ''
  try {
    token = await notebookStore.getAttachmentToken(notebookId)
  } catch { /* proceed without token */ }
  if (!token) {
    renderedContent.value = DOMPurify.sanitize(marked.parse(processed) as string)
    return
  }
  processed = processed.replace(/!\[([^\]]*)\]\((?:<(attachment\/[^>]+)>|(attachment\/[^)]+))\)/g, (_match, alt, path1, path2) => {
    const url = getAttachmentUrl(notebookId, path1 || path2, token)
    return `![${alt}](${url})`
  })
  processed = processed.replace(/(?<!!)\[([^\]]*)\]\((?:<(attachment\/[^>]+)>|(attachment\/[^)]+))\)/g, (_match, text, path1, path2) => {
    const url = getAttachmentUrl(notebookId, path1 || path2, token)
    return `[${text}](${url})`
  })
  renderedContent.value = DOMPurify.sanitize(marked.parse(processed) as string)
  await nextTick()
  const container = document.querySelector('.preview-container')
  if (container) {
    const imgs = container.querySelectorAll('img')
    imgs.forEach(img => {
      img.addEventListener('error', async () => {
            if (img.dataset.retried === 'true') return
            img.dataset.retried = 'true'
            try {
              const resp = await fetch(img.src)
              const data = await resp.json()
              if (data.fail_code === 'TOKEN_EXPIRED') {
                const nbId = noteStore.currentNote?.notebookId
                if (nbId) {
                  notebookStore.clearAttachmentToken(nbId)
                  const newToken = await notebookStore.getAttachmentToken(nbId)
                  if (newToken) {
                    img.src = img.src.replace(/token=[^&]+/, `token=${newToken}`)
                  }
                }
              }
            } catch { /* ignore */ }
          }, { once: true })
    })
  }
}

let renderTimer: ReturnType<typeof setTimeout> | null = null
watch(() => noteStore.currentNote?.content, () => {
  if (renderTimer) clearTimeout(renderTimer)
  renderTimer = setTimeout(() => updateRenderedContent(), 300)
})

function convertTree(tree: FileTreeNode[], notebookId: string, insideAttachment = false): TreeNodeData[] {
  const result: TreeNodeData[] = []
  for (const node of tree) {
    if (node.is_dir) {
      const isAttFolder = node.name === 'attachment'
      result.push({
        id: `folder-${notebookId}-${node.path}`,
        label: isAttFolder ? `[${t('notes.attachmentFolder')}]` : node.name,
        isFolder: true,
        isAttachmentFolder: isAttFolder,
        notebookId,
        path: node.path,
        isLeaf: !node.children || node.children.length === 0,
      })
    } else if (insideAttachment) {
      result.push({
        id: `attachment-${notebookId}-${node.path}`,
        label: node.name,
        isAttachment: true,
        notebookId,
        path: node.path,
        isLeaf: true,
      })
    } else {
      result.push({
        id: `note-${notebookId}-${node.path}`,
        label: node.name.endsWith('.md') ? node.name.slice(0, -3) : node.name,
        isNote: true,
        notebookId,
        path: node.path,
        isLeaf: true,
      })
    }
  }
  return result
}

const treeData = ref<TreeNodeData[]>([])

interface MoveToFolderNode {
  path: string
  label: string
  children?: MoveToFolderNode[]
}

function collectFoldersFromTree(nodes: FileTreeNode[]): MoveToFolderNode[] {
  const result: MoveToFolderNode[] = []
  for (const node of nodes) {
    if (node.is_dir && node.name !== 'attachment') {
      const childFolders = node.children ? collectFoldersFromTree(node.children) : []
      result.push({ path: node.path, label: node.name, children: childFolders.length > 0 ? childFolders : undefined })
    }
  }
  return result
}

const moveToFolderTree = computed(() => {
  const fileTree = notebookStore.fileTreeMap.get(moveToNotebookId.value)
  const rootChildren = fileTree ? collectFoldersFromTree(fileTree) : []
  return [{ path: '', label: t('notes.moveToRoot'), children: rootChildren.length > 0 ? rootChildren : undefined }]
})


const treeProps = { label: 'label' as const, isLeaf: 'isLeaf' as const }

const defaultExpandedKeys = ref<string[]>([])

function checkLayout() { isMobile.value = window.innerWidth < 768 }

function reloadNotebookNode(nbId: string) {
  const node = treeRef.value?.store.nodesMap[nbId]
  if (node) {
    node.loaded = false
    node.expand()
  }
}

function removeTreeNode(nodeId: string) {
  const node = treeRef.value?.store.nodesMap[nodeId]
  if (!node) return
  const parent = node.parent
  treeRef.value?.remove(node.data)
  if (parent && parent.childNodes.length === 0 && !parent.data.isNotebook && !parent.data.isRoot) {
    parent.data.isLeaf = true
  }
}

function renameTreeNode(
  nbId: string, oldPath: string, newPath: string, newLabel: string, isFolder: boolean,
) {
  const prefix = isFolder ? 'folder' : 'note'
  const oldNodeId = `${prefix}-${nbId}-${oldPath}`
  const newNodeId = `${prefix}-${nbId}-${newPath}`
  const store = treeRef.value?.store
  const oldNode = store?.nodesMap[oldNodeId]
  if (!oldNode || !store) return

  const parent = oldNode.parent
  if (!parent) return

  const wasSelected = treeRef.value?.getCurrentKey() === oldNodeId
  const siblings = parent.childNodes
  const oldIndex = siblings.findIndex((n: any) => n.data?.id === oldNodeId)
  const nextSibling = oldIndex >= 0 && oldIndex + 1 < siblings.length ? siblings[oldIndex + 1] : null

  const newData: TreeNodeData = {
    id: newNodeId,
    label: newLabel,
    isFolder: isFolder ? true : undefined,
    isNote: isFolder ? undefined : true,
    notebookId: nbId,
    path: newPath,
    isLeaf: !isFolder,
  }

  treeRef.value?.remove(oldNode.data)

  if (nextSibling) {
    treeRef.value?.insertBefore(newData, nextSibling.data)
  } else {
    treeRef.value?.append(newData, parent.data)
  }

  if (wasSelected) {
    treeRef.value?.setCurrentKey(newNodeId)
  }
}

async function loadNotebookNode(node: any, resolve: (data: TreeNodeData[]) => void) {
  const data: TreeNodeData = node.data
  if (data.isRoot) {
    resolve(notebookStore.notebooks.map(nb => ({
      id: nb.id, label: nb.name, isNotebook: true, notebookId: nb.id, isLeaf: false
    })))
    return
  }
  if (data.isFolder) {
    const fileTree = notebookStore.fileTreeMap.get(data.notebookId!)
    const insideAttachment = data.path!.split('/').includes('attachment')
    if (!fileTree) { resolve([]); return }
    const children = findChildrenByPath(fileTree, data.path!)
    resolve(children ? convertTree(children, data.notebookId!, insideAttachment) : [])
    return
  }
  if (!data.isNotebook) {
    resolve([])
    return
  }
  const nb = notebookStore.getNotebookById(data.id)
  if (!nb) { resolve([]); return }

  if (nb.encrypted && !cryptoStore.isUnlocked(nb.id)) {
    node.expanded = false
    unlockingNotebookId.value = nb.id
    unlockPassword.value = ''
    unlockDialogVisible.value = true
    resolve([])
    return
  }

  const fileTree = await notebookStore.fetchFileTree(nb.id)
  if (fileTree) {
    resolve(convertTree(fileTree, nb.id))
  } else {
    resolve([])
  }
}

function findChildrenByPath(tree: FileTreeNode[], targetPath: string): FileTreeNode[] | null {
  for (const node of tree) {
    if (node.path === targetPath) return node.children || []
    if (node.children) {
      const found = findChildrenByPath(node.children, targetPath)
      if (found) return found
    }
  }
  return null
}

async function selectNotebook(notebook: NotebookItem) {
  if (noteStore.currentNote?.isDirty) {
    try {
      await ElMessageBox.confirm(
        t('notes.unsavedConfirmMessage'),
        t('notes.unsavedConfirmTitle'),
        {
          distinguishCancelAndClose: true,
          confirmButtonText: t('notes.continueEditing'),
          cancelButtonText: t('notes.discardAction'),
          type: 'warning',
        }
      )
      return
    } catch (action) {
      if (action === 'close') return
    }
  }
  if (notebook.encrypted && !cryptoStore.isUnlocked(notebook.id)) {
    unlockingNotebookId.value = notebook.id
    unlockPassword.value = ''
    unlockDialogVisible.value = true
    return
  }
  currentNotebook.value = notebook
  mobileCurrentFolderPath.value = ''
  if (isMobile.value) {
    await notebookStore.fetchFileTree(notebook.id)
  }
}

function handleMobileNodeClick(data: TreeNodeData) {
  if (data.isFolder && !data.isAttachmentFolder) {
    mobileCurrentFolderPath.value = data.path || ''
    return
  }
  if (data.isNote) {
    handleNodeClick(data)
  }
}

async function executeMobileSearch() {
  if (noteStore.currentNote?.isDirty) {
    ElMessage.warning({ __key: 'notes.saveBeforeSearch' })
    return
  }
  if (!mobileSearchKeyword.value.trim()) {
    searchTreeData.value = []
    return
  }
  mobileSearchLoading.value = true
  try {
    const keyword = mobileSearchKeyword.value.trim()
    const apiRes = await searchNotes({ keyword })
    const encryptedResults = await cryptoStore.searchEncryptedNotebooks(keyword)
    const notebookMap = new Map<string, SearchTreeNotebook>()
    for (const r of apiRes.results) {
      let nb = notebookMap.get(r.notebook_id)
      if (!nb) {
        nb = { id: `nb-${r.notebook_id}`, label: r.notebook_name, notebookId: r.notebook_id, encrypted: false, children: [] }
        notebookMap.set(r.notebook_id, nb)
      }
      const matchItems: SearchTreeMatch[] = r.matches.map((m, i) => ({
        id: `match-${r.notebook_id}-${r.note_path}-${i}`,
        lineNumber: m.line_number,
        content: m.content,
      }))
      nb.children.push({
        id: `note-${r.notebook_id}-${r.note_path}`,
        label: r.title,
        notePath: r.note_path,
        matchItems,
      })
    }
    for (const r of encryptedResults) {
      let nb = notebookMap.get(r.notebookId)
      if (!nb) {
        nb = { id: `nb-${r.notebookId}`, label: notebookStore.getNotebookById(r.notebookId)?.name || r.notebookId, notebookId: r.notebookId, encrypted: true, children: [] }
        notebookMap.set(r.notebookId, nb)
      }
      nb.children.push({
        id: `note-${r.notebookId}-${r.notePath}`,
        label: r.title,
        notePath: r.notePath,
        matchItems: [],
      })
    }
    searchTreeData.value = Array.from(notebookMap.values()).sort((a, b) => a.label.localeCompare(b.label))
    expandedSearchGroups.value = new Set(searchTreeData.value.map(nb => nb.id))
  } catch {
    searchTreeData.value = []
  } finally {
    mobileSearchLoading.value = false
  }
}

function clearMobileSearch() {
  mobileSearchActive.value = false
  mobileSearchKeyword.value = ''
  searchTreeData.value = []
}

async function confirmUnsaved(): Promise<boolean> {
  if (!noteStore.currentNote?.isDirty) return true
  try {
    await ElMessageBox.confirm(
      t('notes.unsavedConfirmMessage'),
      t('notes.unsavedConfirmTitle'),
      {
        distinguishCancelAndClose: true,
        confirmButtonText: t('notes.continueEditing'),
        cancelButtonText: t('notes.discardAction'),
        type: 'warning',
      }
    )
    return false
  } catch (action) {
    return action !== 'close'
  }
}

async function handleMobileBack() {
  if (!(await confirmUnsaved())) return
  noteStore.closeNote()
}

async function handleNodeClick(data: TreeNodeData) {
  const cur = noteStore.currentNote
  if (data.isNote && cur && !cur.isDirty && cur.notebookId === data.notebookId && cur.path === data.path) return
  if (cur?.isDirty && data.isNote && cur.notebookId === data.notebookId && cur.path === data.path) return
  if (cur?.isDirty) {
    try {
      await ElMessageBox.confirm(
        t('notes.unsavedConfirmMessage'),
        t('notes.unsavedConfirmTitle'),
        {
          distinguishCancelAndClose: true,
          confirmButtonText: t('notes.continueEditing'),
          cancelButtonText: t('notes.discardAction'),
          type: 'warning',
        }
      )
      treeRef.value?.setCurrentKey(`note-${cur.notebookId}-${cur.path}`)
      return
    } catch (action) {
      if (action === 'close') {
        treeRef.value?.setCurrentKey(`note-${cur.notebookId}-${cur.path}`)
        return
      }
    }
  }
  if (data.isNote) {
    const nb = notebookStore.getNotebookById(data.notebookId || '')
    if (nb) currentNotebook.value = nb
    if (data.notebookId && data.path) {
      const n = notebookStore.getNotebookById(data.notebookId)
      noteStore.openNote(data.notebookId, data.path, n?.encrypted ?? false, data.label)
    }
  } else {
    if (data.isNotebook) {
      const nb = notebookStore.getNotebookById(data.id)
      if (nb) selectNotebook(nb)
    }
    const nbId = data.isNotebook ? data.id : (data.notebookId || '')
    const nb = notebookStore.getNotebookById(nbId)
    if (nb) currentNotebook.value = nb
    noteStore.closeNote()
  }
}

function handleNodeContextMenu(event: MouseEvent, data: TreeNodeData) {
  event.preventDefault()
  contextMenu.x = event.clientX
  contextMenu.y = event.clientY
  contextMenu.isRoot = !!data.isRoot
  contextMenu.isFolder = !!data.isFolder
  contextMenu.isAttachmentFolder = !!data.isAttachmentFolder
  contextMenu.isNote = !!data.isNote
  contextMenu.isAttachment = !!data.isAttachment
  contextMenu.notebookId = data.isNotebook ? data.id : (data.notebookId || '')
  contextMenu.path = data.path || ''
  contextMenu.label = data.label || ''
  contextMenu.visible = true
}

function hideContextMenu() { contextMenu.visible = false }

function handleCreateNotebook() {
  hideContextMenu()
  createNotebookForm.name = ''
  createNotebookForm.description = ''
  createNotebookForm.path = ''
  createNotebookForm.encrypted = false
  createNotebookForm.password = ''
  createNotebookVisible.value = true
}

function handleImportNotebook() {
  hideContextMenu()
  importNotebookForm.name = ''
  importNotebookForm.description = ''
  importNotebookForm.encrypted = false
  importNotebookForm.password = ''
  importNotebookForm.path = ''
  importNotebookVisible.value = true
}

function handleEditNotebook() {
  hideContextMenu()
  const nb = notebookStore.getNotebookById(contextMenu.notebookId)
  if (!nb) return
  editNotebookForm.id = nb.id
  editNotebookForm.name = nb.name
  editNotebookForm.description = nb.description
  editNotebookForm.encrypted = nb.encrypted
  editNotebookForm.path = nb.path
  editNotebookVisible.value = true
}

function handleCloseNotebook() {
  hideContextMenu()
  const nbId = contextMenu.notebookId
  if (!nbId) return
  cryptoStore.lockNotebook(nbId)
  notebookStore.clearAttachmentToken(nbId)
  notebookStore.clearFileTree(nbId)
  if (currentNotebook.value?.id === nbId) {
    currentNotebook.value = null
    noteStore.closeNote()
  }
  const node = treeRef.value?.store.nodesMap[nbId]
  if (node) {
    node.loaded = false
    node.loading = false
    node.expanded = false
  }
  ElMessage.success({ __key: 'notes.closeNotebookSuccess' })
}

async function handleDeleteNotebookCtx() {
  hideContextMenu()
  const nb = notebookStore.getNotebookById(contextMenu.notebookId)
  if (!nb) return
  try { await ElMessageBox.confirm(t('notes.deleteNotebookConfirm', { name: nb.name }), t('common.confirm'), { type: 'warning' }) } catch { return }
  try {
    await notebookStore.removeNotebook(nb.id)
    cryptoStore.lockNotebook(nb.id)
    notebookStore.clearAttachmentToken(nb.id)
    if (currentNotebook.value?.id === nb.id) { currentNotebook.value = null; noteStore.closeNote() }
    reloadNotebookNode('root')
    ElMessage.success({ __key: 'notes.deleteNotebookSuccess' })
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: 'common.error' })
    }
  }
}

function handleCreateNote() {
  hideContextMenu()
  noteFormName.value = ''
  noteParentPath.value = contextMenu.path || ''
  noteNotebookId.value = contextMenu.notebookId
  noteDialogVisible.value = true
}

function handleCreateFolder() {
  hideContextMenu()
  folderFormName.value = ''
  folderParentPath.value = contextMenu.path || ''
  folderNotebookId.value = contextMenu.notebookId
  folderDialogVisible.value = true
}

async function submitFolderForm() {
  if (!folderFormName.value.trim()) { ElMessage.warning({ __key: 'notes.folderNameRequired' }); return }
  if (folderFormName.value.trim().toLowerCase() === 'attachment') { ElMessage.warning({ __key: 'notes.folderNameReserved' }); return }
  folderFormLoading.value = true
  try {
  const nbId = folderNotebookId.value
  const nb = notebookStore.getNotebookById(nbId)
  if (!nb) return
  let folderName = folderFormName.value.trim()
  if (nb.encrypted && cryptoStore.isUnlocked(nbId)) {
    try { folderName = await cryptoStore.encryptPath(nbId, folderName) } catch { ElMessage.error({ __key: 'common.error' }); return }
  }
  const savePath = folderParentPath.value ? `${folderParentPath.value}/${folderName}` : folderName
  const res = await createNotebookFolder(nbId, savePath)
  if (res.success) {
    reloadNotebookNode(nbId)
    folderDialogVisible.value = false
  } else {
    ElMessage.error({ __key: `errors.${res.fail_code}` })
  }
  } finally { folderFormLoading.value = false }
}

function handleRenameNode() {
  hideContextMenu()
  renameNotebookId.value = contextMenu.notebookId
  renamePath.value = contextMenu.path
  renameIsFolder.value = !!contextMenu.isFolder
  const name = (contextMenu.label || contextMenu.path.split('/').pop() || '')
  renameFormName.value = name.replace(/\.md$/, '')
  renameDialogVisible.value = true
}

function handleMoveTo() {
  hideContextMenu()
  moveToNotebookId.value = contextMenu.notebookId
  moveToSourcePath.value = contextMenu.path
  moveToTargetFolder.value = ''
  moveToSourceIsFolder.value = contextMenu.isFolder
  moveToDialogVisible.value = true
}

function handleMoveToFolderClick(data: MoveToFolderNode) {
  moveToTargetFolder.value = data.path
}

async function submitMoveTo() {
  const nbId = moveToNotebookId.value
  const sourcePath = moveToSourcePath.value
  const targetFolder = moveToTargetFolder.value
  if (!nbId || !sourcePath) return

  const normalizedSource = sourcePath.replace(/\\/g, '/')
  const normalizedTarget = targetFolder.replace(/\\/g, '/')
  if (normalizedSource === normalizedTarget) {
    ElMessage.warning({ __key: 'notes.cannotMoveToSelf' })
    return
  }
  const parentPath = normalizedSource.includes('/') ? normalizedSource.substring(0, normalizedSource.lastIndexOf('/')) : ''
  if (parentPath === normalizedTarget) {
    ElMessage.warning({ __key: 'notes.cannotMoveToSelf' })
    return
  }
  if (moveToSourceIsFolder.value && (normalizedTarget === normalizedSource || normalizedTarget.startsWith(normalizedSource + '/'))) {
    ElMessage.warning({ __key: 'notes.cannotMoveToSelf' })
    return
  }

  moveToLoading.value = true
  try {
    const res = await notebookStore.moveNotePath(nbId, sourcePath, targetFolder)
    if (res.success) {
      if (noteStore.currentNote?.notebookId === nbId && noteStore.currentNote?.path === sourcePath) {
        noteStore.currentNote.isDirty = false
        noteStore.closeNote()
      }
      moveToDialogVisible.value = false
      reloadNotebookNode(nbId)
      ElMessage.success({ __key: 'notes.moveToSuccess' })
    } else {
      ElMessage.error({ __key: `errors.${res.fail_code}` })
    }
  } finally {
    moveToLoading.value = false
  }
}

async function handleDeleteNode() {
  hideContextMenu()
  const path = contextMenu.path
  if (!path) return
  const nbId = contextMenu.notebookId
  const nb = notebookStore.getNotebookById(nbId)
  if (!nb) return
  if (contextMenu.isFolder) {
    const fileTree = notebookStore.fileTreeMap.get(nbId)
    const children = fileTree ? findChildrenByPath(fileTree, path) : null
    if (children && children.length > 0) {
      ElMessage.warning({ __key: 'notes.deleteFolderNotEmpty' })
      return
    }
    try { await ElMessageBox.confirm(t('notes.deleteFolderConfirm', { name: contextMenu.label }), t('common.confirm'), { type: 'warning' }) } catch { return }
    const res = await notebookStore.deleteFolder(nbId, path)
    if (res.success) {
      if (noteStore.currentNote?.notebookId === nbId && noteStore.currentNote?.path.startsWith(path + '/')) {
        noteStore.currentNote.isDirty = false
        noteStore.closeNote()
      }
      removeTreeNode(`folder-${nbId}-${path}`)
      ElMessage.success({ __key: 'notes.deleteFolderSuccess' })
    } else {
      ElMessage.error({ __key: `errors.${res.fail_code}` })
    }
  } else {
    try { await ElMessageBox.confirm(t('notes.deleteNoteConfirm', { name: contextMenu.label }), t('common.confirm'), { type: 'warning' }) } catch { return }
    const res = await notebookStore.batchDeleteFiles(nbId, [path])
    if (res.success) {
      if (noteStore.currentNote?.notebookId === nbId && noteStore.currentNote?.path === path) {
        noteStore.currentNote.isDirty = false
        noteStore.closeNote()
      }
      removeTreeNode(`note-${nbId}-${path}`)
      ElMessage.success({ __key: 'notes.deleteNoteSuccess' })
    } else {
      ElMessage.error({ __key: 'common.error' })
    }
  }
}

const handlePaste = async (e: ClipboardEvent) => {
  if (!editorInstance.value?.hasFocus) return
  if (!currentNotebook.value) return
  const items = e.clipboardData?.items
  if (!items) return
  for (const item of items) {
    if (item.type.startsWith('image/')) {
      e.preventDefault()
      e.stopPropagation()
      e.stopImmediatePropagation()
      const file = item.getAsFile()
      if (!file) continue
      const attachmentPath = await uploadAttachment(file, true)
      if (attachmentPath) {
        insertAtCursor(`![image](<${attachmentPath}>)`)
      }
      break
    }
  }
}

function generateUUID(): string {
  return crypto.randomUUID()
}

async function uploadAttachment(file: File, isImage: boolean): Promise<string | null> {
  if (!currentNotebook.value) return null
  const ext = file.name.split('.').pop() || (isImage ? 'png' : 'bin')
  const useOriginalName = !currentNotebook.value.encrypted
  let filename: string
  if (useOriginalName) {
    filename = file.name
  } else {
    filename = `${generateUUID()}.${ext}`
  }
  try {
    let res: { success: boolean; path?: string }
    if (currentNotebook.value.encrypted && cryptoStore.isUnlocked(currentNotebook.value.id)) {
      const arrayBuffer = await file.arrayBuffer()
      const encrypted = await cryptoStore.encryptAttachment(currentNotebook.value.id, arrayBuffer)
      const encryptedBlob = new Blob([encrypted])
      const encryptedFile = new File([encryptedBlob], filename, { type: file.type })
      res = await notebookStore.uploadAttachment(currentNotebook.value.id, filename, encryptedFile)
    } else {
      res = await notebookStore.uploadAttachment(currentNotebook.value.id, filename, file)
    }
    const actualName = res.path || filename
    reloadNotebookNode(currentNotebook.value.id)
    return `attachment/${actualName}`
  } catch (error: any) {
    if (error.isAxiosError && !error.response) {
      ElMessage.error({ __key: isImage ? 'notes.imageUploadFailed' : 'notes.uploadFailed' })
    }
    return null
  }
}

function insertAtCursor(text: string) {
  const view = editorInstance.value
  if (!view) return
  const { from, to } = view.state.selection.main
  view.dispatch({ changes: { from, to, insert: text } })
}

function triggerImageUpload() {
  imageInputRef.value?.click()
}

function triggerFileUpload() {
  fileInputRef.value?.click()
}

async function handleImageUpload(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  input.value = ''
  const attachmentPath = await uploadAttachment(file, true)
  if (attachmentPath) {
    insertAtCursor(`![${file.name}](<${attachmentPath}>)`)
  }
}

async function handleFileUpload(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  input.value = ''
  const attachmentPath = await uploadAttachment(file, false)
  if (attachmentPath) {
    insertAtCursor(`[${file.name}](<${attachmentPath}>)`)
  }
}

function initEditor() {
  if (!editorContainer.value) return
  if (editorInstance.value) { editorInstance.value.destroy(); editorInstance.value = null }
  const state = EditorState.create({
    doc: noteStore.currentNote?.content || '',
    extensions: [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      foldGutter(),
      drawSelection(),
      indentOnInput(),
      bracketMatching(),
      closeBrackets(),
      autocompletion(),
      rectangularSelection(),
      highlightActiveLine(),
      highlightSelectionMatches(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      keymap.of([
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...searchKeymap,
        ...historyKeymap,
        ...completionKeymap,
        indentWithTab,
      ]),
      markdown({ codeLanguages: languages }),
      EditorView.lineWrapping,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          noteStore.updateContent(update.state.doc.toString())
        }
      }),
      themeCompartment.of(themeStore.isDark ? oneDark : []),
      EditorView.theme({
        '&': { height: '100%', fontSize: '14px' },
        '.cm-scroller': { overflow: 'auto' },
      }),
    ],
  })
  editorInstance.value = new EditorView({ state, parent: editorContainer.value })
  document.addEventListener('paste', handlePaste, true)
}

function updateEditorTheme() {
  if (!editorInstance.value) return
  themeCompartment.reconfigure(themeStore.isDark ? oneDark : [])
}

function insertFormat(type: string) {
  if (!editorInstance.value) return
  const view = editorInstance.value
  const { from, to } = view.state.selection.main
  const selectedText = view.state.sliceDoc(from, to)
  let insertText = ''
  let cursorOffset = 0
  switch (type) {
    case 'bold': insertText = `**${selectedText || 'bold text'}**`; cursorOffset = selectedText ? 0 : 2; break
    case 'italic': insertText = `*${selectedText || 'italic text'}*`; cursorOffset = selectedText ? 0 : 1; break
    case 'link': insertText = `[${selectedText || 'link text'}](url)`; cursorOffset = selectedText ? 0 : 1; break
    case 'unorderedList': insertText = `- ${selectedText || 'list item'}`; break
    case 'orderedList': insertText = `1. ${selectedText || 'list item'}`; break
    case 'heading': insertText = `## ${selectedText || 'Heading'}`; break
    case 'horizontalRule': insertText = '\n---\n'; break
    case 'table': insertText = `\n| ${t('notes.tableCol1')} | ${t('notes.tableCol2')} | ${t('notes.tableCol3')} |\n| --- | --- | --- |\n| ${t('notes.tableContent')} | ${t('notes.tableContent')} | ${t('notes.tableContent')} |\n`; break
    default: return
  }
  const anchor = from + insertText.length - cursorOffset
  view.dispatch({
    changes: { from, to, insert: insertText },
    selection: type !== 'horizontalRule' && selectedText
      ? { anchor: from, head: from + insertText.length }
      : { anchor },
  })
  view.focus()
}

async function handleSaveNote() {
  if (!currentNotebook.value) return
  const res = await noteStore.saveCurrentNote(currentNotebook.value.encrypted)
  if (res.success) {
    ElMessage.success({ __key: 'notes.saveSuccess' })
  } else if (res.fail_code === 'CONFLICT_DETECTED') {
    const nb = currentNotebook.value
    const note = noteStore.currentNote
    if (!note) return
    if (nb.encrypted) {
      try {
        await ElMessageBox.confirm(
          t('notes.encryptedConflictMessage'),
          t('notes.conflictTitle'),
          {
            confirmButtonText: t('notes.discardAction'),
            cancelButtonText: t('notes.continueEditing'),
            type: 'warning',
          }
        )
        note.isDirty = false
        await noteStore.openNote(nb.id, note.path, true, note.title)
      } catch { /* user chose to continue editing */ }
      return
    }
    try {
      const conflictRes = await noteStore.saveConflictFile(nb.id, note.path, note.content)
      if (conflictRes.success && conflictRes.conflict_path) {
        const conflictTitle = conflictRes.conflict_path.replace(/\.md$/, '').split('/').pop() || conflictRes.conflict_path
        note.path = conflictRes.conflict_path
        note.hash = conflictRes.hash || null
        note.isDirty = false
        note.title = conflictTitle
        await reloadNotebookNode(nb.id)
        treeRef.value?.setCurrentKey(`note-${nb.id}-${conflictRes.conflict_path}`)
        ElMessage.success({ __key: 'notes.conflictAutoSaved' })
      } else {
        ElMessage.error({ __key: conflictRes.fail_code ? `errors.${conflictRes.fail_code}` : 'common.error' })
      }
    } catch {
      ElMessage.error({ __key: 'common.error' })
    }
  } else {
    ElMessage.error({ __key: `errors.${res.fail_code}` })
  }
}

watch(() => themeStore.isDark, () => updateEditorTheme())

watch(() => noteStore.currentNote, (note) => {
  mobilePreviewMode.value = false
  if (note && !note.isLoading) {
    nextTick(() => initEditor())
  } else if (editorInstance.value) {
    document.removeEventListener('paste', handlePaste, true)
    editorInstance.value.destroy()
    editorInstance.value = null
  }
}, { immediate: true })

watch(() => noteStore.currentNote?.isLoading, (loading) => {
  if (loading === false && noteStore.currentNote) {
    nextTick(() => {
      if (!editorInstance.value) {
        initEditor()
      } else {
        editorInstance.value.dispatch({
          changes: { from: 0, to: editorInstance.value.state.doc.length, insert: noteStore.currentNote?.content || '' }
        })
      }
    })
  }
})

async function submitCreateNotebook() {
  if (!createNotebookFormRef.value) return
  const valid = await createNotebookFormRef.value.validate().catch(() => false)
  if (!valid) return
  createNotebookLoading.value = true
  try {
    let signature: string | undefined
    if (createNotebookForm.encrypted) {
      const sigContent = await cryptoStore.generateSignatureFile(createNotebookForm.password)
      signature = JSON.stringify(sigContent)
    }
    const res = await notebookStore.createNotebook({
      name: createNotebookForm.name,
      description: createNotebookForm.description,
      path: createNotebookForm.path,
      encrypted: createNotebookForm.encrypted || undefined,
      signature,
    })
    if (res.success && res.id) {
      if (createNotebookForm.encrypted && signature) {
        const unlocked = await cryptoStore.unlockNotebook(res.id, createNotebookForm.password, signature)
        if (!unlocked) {
          await notebookStore.removeNotebook(res.id)
          ElMessage.error({ __key: 'notes.unlockFailed' })
          return
        }
      }
      createNotebookVisible.value = false
      reloadNotebookNode('root')
      ElMessage.success({ __key: 'notes.createNotebookSuccess' })
    } else if (res.fail_code) {
      ElMessage.error({ __key: `errors.${res.fail_code}` })
    }
  } finally { createNotebookLoading.value = false }
}

async function submitImportNotebook() {
  if (!importNotebookFormRef.value) return
  const valid = await importNotebookFormRef.value.validate().catch(() => false)
  if (!valid) return
  importNotebookLoading.value = true
  try {
    const encrypted = importNotebookForm.encrypted
    const res = await notebookStore.openNotebook({ name: importNotebookForm.name, description: importNotebookForm.description, path: importNotebookForm.path, encrypted })
    if (!res.success) {
      if (res.fail_code) { ElMessage.error({ __key: `errors.${res.fail_code}` }) }
      return
    }
    if (!res.id) return
    if (encrypted) {
      const sigRes = await readNoteApi({ notebook_id: res.id, path: '.notebook.sig' })
      if (!('success' in sigRes) || !sigRes.success) {
        await notebookStore.removeNotebook(res.id)
        ElMessage.error({ __key: 'notes.wrongPassword' })
        return
      }
      const validPw = await cryptoStore.unlockNotebook(res.id, importNotebookForm.password, sigRes.content)
      if (!validPw) {
        await notebookStore.removeNotebook(res.id)
        ElMessage.error({ __key: 'notes.wrongPassword' })
        return
      }
    }
    importNotebookVisible.value = false
    reloadNotebookNode('root')
    ElMessage.success({ __key: 'notes.importNotebookSuccess' })
  } finally { importNotebookLoading.value = false }
}

async function submitEditNotebook() {
  if (!editNotebookForm.id) return
  const formEl = editNotebookFormRef.value
  if (formEl) { try { await formEl.validate() } catch { return } }
  editNotebookLoading.value = true
  try {
    await notebookStore.updateNotebookData(editNotebookForm.id, editNotebookForm.name, editNotebookForm.description)
    editNotebookVisible.value = false
    reloadNotebookNode('root')
    if (currentNotebook.value?.id === editNotebookForm.id) {
      const updated = notebookStore.getNotebookById(editNotebookForm.id)
      if (updated) currentNotebook.value = updated
    }
    ElMessage.success({ __key: 'notes.editNotebookSuccess' })
  } finally { editNotebookLoading.value = false }
}

async function submitUnlock() {
  const nbId = unlockingNotebookId.value
  if (!nbId) return
  const nb = notebookStore.getNotebookById(nbId)
  if (!nb) return
  unlockLoading.value = true
  try {
  try {
    const sigRes = await readNoteApi({ notebook_id: nbId, path: '.notebook.sig' })
    if ('success' in sigRes && sigRes.success && 'hash' in sigRes) {
      const valid = await cryptoStore.unlockNotebook(nbId, unlockPassword.value, sigRes.content)
      if (!valid) { ElMessage.error({ __key: 'notes.wrongPassword' }); return }
    } else {
      ElMessage.error({ __key: 'notes.wrongPassword' }); return
    }
  } catch {
    ElMessage.error({ __key: 'notes.wrongPassword' }); return
  }
  unlockDialogVisible.value = false
  currentNotebook.value = nb
  noteStore.closeNote()
  mobileCurrentFolderPath.value = ''
  if (isMobile.value) {
    await notebookStore.fetchFileTree(nb.id)
  } else {
    nextTick(() => {
      const node = treeRef.value?.store.nodesMap[nbId]
      if (node) {
        node.loaded = false
        node.expand()
      }
    })
  }
  ElMessage.success({ __key: 'notes.unlockNotebookSuccess' })
  } finally { unlockLoading.value = false }
}

async function handleDownloadAttachment() {
  hideContextMenu()
  const nb = notebookStore.getNotebookById(contextMenu.notebookId)
  if (!nb || !contextMenu.path) return
  try {
    const token = await notebookStore.getAttachmentToken(nb.id)
    const url = getAttachmentUrl(nb.id, contextMenu.path, token)
    const a = document.createElement('a')
    a.href = url
    a.download = contextMenu.label || contextMenu.path.split('/').pop() || 'attachment'
    a.click()
  } catch {
    ElMessage.error({ __key: 'notes.downloadFailed' })
  }
}

async function handleDeleteAttachment() {
  hideContextMenu()
  const nb = notebookStore.getNotebookById(contextMenu.notebookId)
  if (!nb || !contextMenu.path) return
  const name = contextMenu.label || contextMenu.path.split('/').pop() || ''
  try { await ElMessageBox.confirm(t('notes.deleteAttachmentConfirm', { name }), t('common.confirm'), { type: 'warning' }) } catch { return }
  const res = await notebookStore.batchDeleteFiles(contextMenu.notebookId, [contextMenu.path])
  if (res.success) {
    ElMessage.success({ __key: 'notes.deleteAttachmentSuccess' })
    removeTreeNode(`attachment-${contextMenu.notebookId}-${contextMenu.path}`)
  } else {
    ElMessage.error({ __key: 'notes.deleteAttachmentFailed' })
  }
}

async function handleAttachmentCleanup() {
  hideContextMenu()
  const nb = notebookStore.getNotebookById(contextMenu.notebookId)
  if (!nb) return
  const tree = notebookStore.fileTreeMap.get(contextMenu.notebookId)
  if (!tree) return

  const notePaths: string[] = []
  const attachmentPaths: string[] = []

  function collectFromTree(nodes: FileTreeNode[]) {
    for (const node of nodes) {
      if (node.is_dir) {
        if (node.path === 'attachment' || node.path.endsWith('/attachment')) {
          if (node.children) {
            for (const child of node.children) {
              if (!child.is_dir) attachmentPaths.push(child.path)
            }
          }
        }
        if (node.children) collectFromTree(node.children)
      } else {
        if (node.name.endsWith('.md')) notePaths.push(node.path)
      }
    }
  }
  collectFromTree(tree)

  cleanupChecked.value = 0
  cleanupTotal.value = notePaths.length
  cleanupUnreferenced.value = []
  cleanupNotebookId.value = contextMenu.notebookId
  cleanupPhase.value = 'checking'
  cleanupDialogVisible.value = true

  const referencedNames = new Set<string>()
  for (const notePath of notePaths) {
    try {
      const noteRes = await readNoteApi({ notebook_id: contextMenu.notebookId, path: notePath })
      if (!('success' in noteRes) || !noteRes.success) { cleanupChecked.value++; continue }
      let content = noteRes.content as string
      if (nb.encrypted && cryptoStore.isUnlocked(contextMenu.notebookId)) {
          try { content = await cryptoStore.decryptContent(contextMenu.notebookId, notePath, content) } catch { /* skip */ }
      }
      const imgRegex = /!\[.*?\]\((?:<(?:\.\/)?attachment\/([^>]+)>|(?:\.\/)?attachment\/([^/?)]+))(?:\?[^)]*)?\)/g
      const linkRegex = /(?<!!)\[.*?\]\((?:<(?:\.\/)?attachment\/([^>]+)>|(?:\.\/)?attachment\/([^/?)]+))(?:\?[^)]*)?\)/g
      let m: RegExpExecArray | null
      while ((m = imgRegex.exec(content)) !== null) { const name = m[1] || m[2]; if (name) referencedNames.add(name) }
      while ((m = linkRegex.exec(content)) !== null) { const name = m[1] || m[2]; if (name) referencedNames.add(name) }
    } catch { /* skip unreadable notes */ }
    cleanupChecked.value++
  }

  const unreferenced = attachmentPaths.filter(p => {
    const filename = p.split('/').pop() || ''
    return !referencedNames.has(filename)
  })

  if (unreferenced.length === 0) {
    cleanupPhase.value = 'done'
    return
  }

  cleanupUnreferenced.value = unreferenced
  cleanupDisplayNames.value = unreferenced.map(p => p.split('/').pop() || p)
  cleanupPhase.value = 'confirm'
}

async function executeCleanup() {
  cleanupPhase.value = 'cleaning'
  const res = await notebookStore.batchDeleteFiles(cleanupNotebookId.value, cleanupUnreferenced.value)
  if (res.success) {
    for (const p of cleanupUnreferenced.value) {
      removeTreeNode(`attachment-${cleanupNotebookId.value}-${p}`)
    }
    cleanupPhase.value = 'done'
  } else {
    ElMessage.error({ __key: 'notes.attachmentCleanupFailed' })
    cleanupDialogVisible.value = false
  }
}

async function executeSearch() {
  if (noteStore.currentNote?.isDirty) {
    ElMessage.warning({ __key: 'notes.saveBeforeSearch' })
    return
  }
  if (!searchKeyword.value.trim()) {
    if (isSearchActive.value) clearSearch()
    return
  }
  searchLoading.value = true
  isSearchActive.value = true
  try {
    const keyword = searchKeyword.value.trim()
    const apiRes = await searchNotes({ keyword })
    const encryptedResults = await cryptoStore.searchEncryptedNotebooks(keyword)

    const notebookMap = new Map<string, SearchTreeNotebook>()

    for (const r of apiRes.results) {
      let nb = notebookMap.get(r.notebook_id)
      if (!nb) {
        nb = { id: `nb-${r.notebook_id}`, label: r.notebook_name, notebookId: r.notebook_id, encrypted: false, children: [] }
        notebookMap.set(r.notebook_id, nb)
      }
      const matchItems: SearchTreeMatch[] = r.matches.map((m, i) => ({
        id: `match-${r.notebook_id}-${r.note_path}-${i}`,
        lineNumber: m.line_number,
        content: m.content,
      }))
      nb.children.push({
        id: `note-${r.notebook_id}-${r.note_path}`,
        label: r.title,
        notePath: r.note_path,
        matchItems,
      })
    }

    for (const r of encryptedResults) {
      let nb = notebookMap.get(r.notebookId)
      if (!nb) {
        const nbInfo = notebookStore.getNotebookById(r.notebookId)
        nb = { id: `nb-${r.notebookId}`, label: nbInfo?.name || '', notebookId: r.notebookId, encrypted: true, children: [] }
        notebookMap.set(r.notebookId, nb)
      }
      nb.children.push({
        id: `note-${r.notebookId}-${r.notePath}`,
        label: r.title,
        notePath: r.notePath,
        matchItems: [],
      })
    }

    searchTreeData.value = Array.from(notebookMap.values()).sort((a, b) => a.label.localeCompare(b.label))
    expandedSearchGroups.value = new Set(searchTreeData.value.map(nb => nb.id))
  } catch {
    isSearchActive.value = false
    searchTreeData.value = []
  } finally {
    searchLoading.value = false
  }
}

function renderMatchContent(content: string): string {
  const firstMatchIdx = content.indexOf('<match>')
  let displayContent = content
  if (firstMatchIdx > 5) {
    const prefix = [...content.substring(0, firstMatchIdx)]
    displayContent = prefix.slice(-5).join('') + content.substring(firstMatchIdx)
  }
  const escaped = displayContent
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  const result = escaped
    .replace(/&lt;match&gt;/g, '<span style="color: var(--el-color-danger); font-weight: 600;">')
    .replace(/&lt;\/match&gt;/g, '</span>')
  return DOMPurify.sanitize(result)
}

function clearSearch() {
  isSearchActive.value = false
  searchTreeData.value = []
  expandedSearchGroups.value = new Set()
  searchKeyword.value = ''
}

function toggleSearchGroup(id: string) {
  const s = new Set(expandedSearchGroups.value)
  if (s.has(id)) s.delete(id); else s.add(id)
  expandedSearchGroups.value = s
}

async function openSearchResultItem(notebookId: string, path: string, encrypted: boolean, lineNumber: number | undefined, title?: string) {
  if (!(await confirmUnsaved())) return
  const nb = notebookStore.getNotebookById(notebookId)
  if (!nb) return
  currentNotebook.value = nb
  const node = treeRef.value?.store.nodesMap[notebookId]
  if (node && !node.expanded) {
    node.expand()
  }
  await noteStore.openNote(notebookId, path, encrypted, title)
  if (lineNumber !== undefined && editorInstance.value) {
    const targetLine = lineNumber + 1
    nextTick(() => {
      const view = editorInstance.value
      if (!view) return
      const lineInfo = view.state.doc.line(targetLine)
      view.dispatch({
        selection: { anchor: lineInfo.from },
        effects: EditorView.scrollIntoView(lineInfo.from, { y: 'center' }),
      })
      view.focus()
    })
  }
}

async function submitNoteForm() {
  if (!noteFormName.value.trim()) { ElMessage.warning({ __key: 'notes.noteNameRequired' }); return }
  noteFormLoading.value = true
  try {
  const nbId = noteNotebookId.value
  const nb = notebookStore.getNotebookById(nbId)
  if (!nb) return
  const encrypted = nb.encrypted
  let noteName = `${noteFormName.value.trim()}.md`
  if (encrypted && cryptoStore.isUnlocked(nbId)) {
    try { noteName = await cryptoStore.encryptPath(nbId, noteName) } catch { ElMessage.error({ __key: 'common.error' }); return }
  }
  const savePath = noteParentPath.value ? `${noteParentPath.value}/${noteName}` : noteName
  let contentToSave = ''
  if (encrypted && cryptoStore.isUnlocked(nbId)) {
    try { contentToSave = await cryptoStore.encryptContent(nbId, savePath, '') } catch { ElMessage.error({ __key: 'common.error' }); return }
  }
    const res = await apiSaveNote({ notebook_id: nbId, path: savePath, content: contentToSave })
  if (res.success) {
    reloadNotebookNode(nbId)
    noteDialogVisible.value = false
    noteStore.openNote(nbId, savePath, encrypted, noteFormName.value.trim())
    ElMessage.success({ __key: 'notes.createNoteSuccess' })
  } else {
    ElMessage.error({ __key: `errors.${res.fail_code}` })
  }
  } finally { noteFormLoading.value = false }
}

async function submitRename() {
  if (!renameFormName.value.trim()) { ElMessage.warning({ __key: 'notes.noteNameRequired' }); return }
  renameFormLoading.value = true
  try {
  const nbId = renameNotebookId.value
  const oldPath = renamePath.value
  const nb = notebookStore.getNotebookById(nbId)
  const encrypted = nb?.encrypted ?? false
  const isFolder = renameIsFolder.value

  let newName = renameFormName.value.trim()
  if (!isFolder) newName = `${newName}.md`

  let encryptedNewName = newName
  if (encrypted && cryptoStore.isUnlocked(nbId)) {
    try {
      encryptedNewName = (await cryptoStore.encryptPath(nbId, newName))
    } catch { ElMessage.error({ __key: 'common.error' }); return }
  }
  const res = await notebookStore.renameNotePath(nbId, oldPath, encryptedNewName)
  if (res.success) {
    const newServerPath = res.new_path
    if (isFolder) {
      if (noteStore.currentNote?.notebookId === nbId && noteStore.currentNote?.path.startsWith(oldPath + '/')) {
        noteStore.currentNote.isDirty = false
        noteStore.closeNote()
      }
    } else if (noteStore.currentNote?.notebookId === nbId && noteStore.currentNote?.path === oldPath) {
      const openPath = newServerPath || newName
      noteStore.currentNote.isDirty = false
      noteStore.closeNote()
      noteStore.openNote(nbId, openPath, encrypted, renameFormName.value.trim())
    }
    renameDialogVisible.value = false
    const newPath = newServerPath || (oldPath.includes('/') ? oldPath.substring(0, oldPath.lastIndexOf('/') + 1) + newName : newName)
    renameTreeNode(nbId, oldPath, newPath, renameFormName.value.trim(), isFolder)
    ElMessage.success({ __key: 'notes.renameNoteSuccess' })
  } else {
    ElMessage.error({ __key: `errors.${res.fail_code}` })
  }
  } finally { renameFormLoading.value = false }
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault()
    if (noteStore.currentNote?.isSaving) return
    handleSaveNote()
  }
}

function handleBeforeUnload(e: BeforeUnloadEvent) {
  if (noteStore.currentNote?.isDirty) {
    e.preventDefault()
  }
}

watch(isMobile, () => {
  mobileCurrentFolderPath.value = ''
  mobileSearchActive.value = false
  mobileSearchKeyword.value = ''
})

onMounted(async () => {
  checkLayout()
  window.addEventListener('resize', checkLayout)
  window.addEventListener('keydown', handleKeydown)
  window.addEventListener('beforeunload', handleBeforeUnload)
  await notebookStore.fetchNotebooks()
  treeData.value = [{ id: 'root', label: t('home.categories.notes'), isRoot: true, isLeaf: false }]
  nextTick(() => {
    const rootNode = treeRef.value?.store.nodesMap['root']
    if (rootNode) {
      rootNode.loaded = false
      rootNode.expand()
    }
  })
})

onUnmounted(() => {
  window.removeEventListener('resize', checkLayout)
  window.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('beforeunload', handleBeforeUnload)
  document.removeEventListener('paste', handlePaste, true)
  if (editorInstance.value) { editorInstance.value.destroy(); editorInstance.value = null }
  if (renderTimer) { clearTimeout(renderTimer); renderTimer = null }
})
</script>

<style scoped>
.notes-container { height: 100%; display: flex; flex: 1; min-height: 0; flex-direction: row; }
.mobile-view { display: flex; flex-direction: column; width: 100%; height: 100%; overflow: hidden; }
.desktop-view { display: flex; flex: 1; min-height: 0; }
.notebook-panel { width: 260px; flex-shrink: 0; border-right: 1px solid var(--el-border-color-lighter); display: flex; flex-direction: column; overflow: hidden; }
.tree-search { padding: 12px; flex-shrink: 0; display: flex; gap: 8px; }
.tree-scroll { flex: 1; min-height: 0; overflow: auto; background-color: var(--el-bg-color); }
.tree-node { display: flex; align-items: center; gap: 8px; font-size: 14px; white-space: nowrap; }
.tree-node-locked { color: var(--el-text-color-secondary); }
.tree-node-unlocked { color: var(--el-color-primary); }
.tree-node-folder { color: var(--el-color-warning); }
.tree-node-folder.attachment-folder { color: var(--el-text-color-secondary); }
.attachment-folder-label { color: var(--el-text-color-secondary); }
.tree-node-note { color: var(--el-color-success); }
.note-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.note-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; color: var(--el-text-color-secondary); gap: 12px; }
.context-menu { position: fixed; z-index: 2000; background: var(--el-bg-color-overlay); border: 1px solid var(--el-border-color-light); border-radius: 4px; padding: 4px 0; box-shadow: var(--el-box-shadow-light); }
.context-menu-item { display: flex; align-items: center; gap: 8px; padding: 8px 16px; cursor: pointer; font-size: 14px; color: var(--el-text-color-regular); white-space: nowrap; }
.context-menu-item:hover { background: var(--el-fill-color-light); color: var(--el-color-primary); }
.context-menu-danger:hover { background: var(--el-color-danger-light-9); color: var(--el-color-danger); }
.context-menu-divider { height: 1px; margin: 4px 0; background: var(--el-border-color-lighter); }
.move-to-tree { width: 100%; }
.mobile-notebook-list { flex: 1; overflow-y: auto; padding: 12px; }
.mobile-notebook-card { display: flex; align-items: center; gap: 12px; padding: 14px 16px; background: var(--el-fill-color-lighter); border-radius: 8px; margin-bottom: 8px; cursor: pointer; }
.mobile-notebook-card:active { background: var(--el-fill-color); }
.mobile-notebook-icon { width: 40px; height: 40px; display: flex; align-items: center; justify-content: center; background: var(--el-color-primary-light-9); border-radius: 8px; color: var(--el-color-primary); flex-shrink: 0; }
.mobile-notebook-info { flex: 1; min-width: 0; }
.mobile-notebook-name { font-size: 15px; font-weight: 500; color: var(--el-text-color-primary); }
.mobile-notebook-desc { font-size: 12px; color: var(--el-text-color-secondary); margin-top: 2px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mobile-notebook-arrow { color: var(--el-text-color-placeholder); flex-shrink: 0; }
.mobile-empty { text-align: center; color: var(--el-text-color-secondary); padding: 40px 0; font-size: 14px; }
.mobile-note-view { display: flex; flex-direction: column; width: 100%; height: 100%; overflow: hidden; position: relative; }
.mobile-note-toolbar { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; flex-shrink: 0; }
.mobile-note-back { display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 15px; font-weight: 500; color: var(--el-text-color-primary); flex: 1; min-width: 0; overflow: hidden; }
.mobile-note-back span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mobile-note-back:active { color: var(--el-text-color-secondary); }
.mobile-tree-list { flex: 1; overflow-y: auto; padding: 0 12px; }
.mobile-tree-item { display: flex; align-items: center; gap: 10px; padding: 12px 14px; background: var(--el-fill-color-lighter); border-radius: 8px; margin-bottom: 6px; cursor: pointer; font-size: 14px; color: var(--el-text-color-primary); }
.mobile-tree-item-attachment { cursor: default; color: var(--el-text-color-secondary); opacity: 0.6; }
.mobile-tree-item-folder { justify-content: flex-start; }
.mobile-tree-item-arrow { margin-left: auto; color: var(--el-text-color-placeholder); font-size: 14px; }
.mobile-tree-item:active { background: var(--el-fill-color); }
.mobile-search-btn { font-size: 20px; color: var(--el-text-color-regular); cursor: pointer; flex-shrink: 0; padding: 4px; }
.mobile-search-input-wrap { padding: 8px 12px; flex-shrink: 0; }
.mobile-editor { flex: 1; display: flex; min-height: 0; padding: 0; }
.mobile-textarea { flex: 1; border: none; outline: none; resize: none; padding: 12px; font-family: var(--el-font-family); font-size: 14px; line-height: 1.6; background: transparent; color: var(--el-text-color-primary); }
.mobile-preview { flex: 1; overflow-y: auto; min-height: 0; }
.mobile-fab { position: absolute; right: 20px; bottom: 24px; width: 48px; height: 48px; border-radius: 50%; background: var(--el-color-primary); color: #fff; display: flex; align-items: center; justify-content: center; cursor: pointer; box-shadow: var(--el-box-shadow-light); z-index: 10; }
.mobile-fab:active { opacity: 0.85; }
.mobile-note-content { flex: 1; display: flex; align-items: center; justify-content: center; }
.encrypt-switch-row { display: flex; align-items: center; gap: 12px; }
.encrypt-switch-label { font-size: 14px; color: var(--el-text-color-regular); }
.editor-container { flex: 1; display: flex; flex-direction: column; min-height: 0; overflow: hidden; }
.editor-header { padding: 12px 16px; border-bottom: 1px solid var(--el-border-color-lighter); flex-shrink: 0; display: flex; align-items: center; gap: 12px; }
.note-title { font-size: 16px; font-weight: 600; color: var(--el-text-color-primary); }
.save-status { font-size: 12px; color: var(--el-text-color-secondary); }
.save-status.unsaved { color: var(--el-text-color-secondary); }
.editor-wrapper { flex: 1; display: flex; min-height: 0; overflow: hidden; }
.editor-pane { flex: 1; display: flex; flex-direction: column; min-width: 0; border-right: 1px solid var(--el-border-color-lighter); }
.editor-toolbar { display: flex; align-items: center; gap: 2px; padding: 6px 10px; background: var(--el-fill-color-lighter); border-bottom: 1px solid var(--el-border-color-lighter); flex-shrink: 0; }
.toolbar-btn { display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 4px; cursor: pointer; color: var(--el-text-color-regular); transition: all 0.2s; }
.toolbar-btn:hover { background: var(--el-fill-color); color: var(--el-color-primary); }
.toolbar-divider { width: 1px; height: 18px; background: var(--el-border-color-lighter); margin: 0 6px; }
.editor-container { flex: 1; min-height: 0; }
.editor-container .cm-editor { height: 100%; }
.editor-container .cm-editor .cm-scroller { overflow: auto; }
.editor-divider { width: 1px; background: var(--el-border-color-lighter); flex-shrink: 0; }
.preview-pane { flex: 1; display: flex; flex-direction: column; min-width: 0; overflow: hidden; }
.preview-container { flex: 1; overflow-y: auto; padding: 16px; color: var(--el-text-color-primary); line-height: 1.6; }
.preview-container :deep(h1), .preview-container :deep(h2), .preview-container :deep(h3), .preview-container :deep(h4), .preview-container :deep(h5), .preview-container :deep(h6) { margin-top: 24px; margin-bottom: 16px; font-weight: 600; line-height: 1.25; color: var(--el-text-color-primary); }
.preview-container :deep(h1) { font-size: 2em; border-bottom: 1px solid var(--el-border-color-lighter); padding-bottom: .3em; }
.preview-container :deep(h2) { font-size: 1.5em; border-bottom: 1px solid var(--el-border-color-lighter); padding-bottom: .3em; }
.preview-container :deep(h3) { font-size: 1.25em; }
.preview-container :deep(h4) { font-size: 1em; }
.preview-container :deep(h5) { font-size: .875em; }
.preview-container :deep(h6) { font-size: .85em; color: var(--el-text-color-secondary); }
.preview-container :deep(p) { margin-bottom: 16px; }
.preview-container :deep(code) { padding: .2em .4em; margin: 0; font-size: 85%; background-color: var(--el-fill-color-light); border-radius: 6px; }
.preview-container :deep(pre) { padding: 16px; overflow: auto; font-size: 85%; line-height: 1.45; background-color: var(--el-fill-color-light); border-radius: 6px; margin-bottom: 16px; }
.preview-container :deep(pre code) { background-color: transparent; padding: 0; }
.preview-container :deep(ul), .preview-container :deep(ol) { padding-left: 2em; margin-bottom: 16px; }
.preview-container :deep(li) { margin-bottom: 4px; }
.preview-container :deep(blockquote) { padding: 0 1em; color: var(--el-text-color-secondary); border-left: .25em solid var(--el-border-color); margin-bottom: 16px; }
.preview-container :deep(a) { color: var(--el-color-primary); text-decoration: none; }
.preview-container :deep(a:hover) { text-decoration: underline; }
.preview-container :deep(img) { max-width: 100%; box-sizing: content-box; background-color: var(--el-bg-color); }
.preview-container :deep(table) { border-spacing: 0; border-collapse: collapse; margin-bottom: 16px; width: 100%; }
.preview-container :deep(table td), .preview-container :deep(table th) { padding: 6px 13px; border: 1px solid var(--el-border-color-lighter); }
.preview-container :deep(table th) { font-weight: 600; background-color: var(--el-fill-color-light); }
.preview-container :deep(table tr:nth-child(2n)) { background-color: var(--el-fill-color-lighter); }
.preview-container :deep(hr) { height: .25em; padding: 0; margin: 24px 0; background-color: var(--el-border-color); border: 0; }
.search-tree-scroll { padding: 4px 0; }
.search-tree-group { margin-bottom: 2px; }
.search-tree-notebook { display: flex; align-items: center; gap: 6px; padding: 6px 12px; cursor: pointer; font-size: 13px; font-weight: 600; color: var(--el-text-color-primary); }
.search-tree-notebook:hover { background: var(--el-fill-color-light); }
.search-tree-arrow { margin-left: auto; transition: transform 0.2s; font-size: 12px; color: var(--el-text-color-secondary); }
.search-tree-arrow.is-expanded { transform: rotate(90deg); }
.search-tree-children { padding-left: 12px; }
.search-tree-note { }
.search-tree-note-title { display: flex; align-items: center; gap: 6px; padding: 5px 12px; cursor: pointer; font-size: 13px; color: var(--el-text-color-regular); }
.search-tree-note-title:hover { background: var(--el-fill-color-light); color: var(--el-color-primary); }
.search-tree-match { display: flex; align-items: center; gap: 6px; padding: 3px 12px 3px 32px; cursor: pointer; font-size: 12px; color: var(--el-text-color-secondary); }
.search-tree-match:hover { background: var(--el-fill-color-light); }
.search-match-line { flex-shrink: 0; color: var(--el-text-color-placeholder); font-family: monospace; }
.search-match-snippet { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.search-empty { text-align: center; color: var(--el-text-color-secondary); padding: 20px; font-size: 14px; }
</style>
