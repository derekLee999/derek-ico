<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { ElMessage } from 'element-plus'
import { Check, Download } from '@element-plus/icons-vue'

interface IconData {
  size: number
  width: number
  height: number
  data_url: string
}

const props = defineProps<{
  icons: IconData[]
  selectedIcon: IconData | null
  fileName: string
}>()

const emit = defineEmits<{
  select: [icon: IconData]
}>()

// ---- Right-click context menu state ----
const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  icon: null as IconData | null,
})

function selectIcon(icon: IconData) {
  emit('select', icon)
}

function onContextMenu(e: MouseEvent, icon: IconData) {
  e.preventDefault()
  selectIcon(icon)
  contextMenu.visible = true
  contextMenu.x = e.clientX
  contextMenu.y = e.clientY
  contextMenu.icon = icon
}

function closeContextMenu() {
  contextMenu.visible = false
}

// Close menu on any click outside
function onWindowClick() {
  if (contextMenu.visible) {
    contextMenu.visible = false
  }
}

// Watch for window clicks to close the menu
if (typeof window !== 'undefined') {
  window.addEventListener('click', onWindowClick)
}

// ---- Download handlers ----
const baseName = computed(() => (props.fileName || 'icon').replace(/\.[^.]+$/, ''))

async function downloadDirect() {
  const icon = contextMenu.icon
  if (!icon) return
  closeContextMenu()

  console.log('[IconGrid] 直接下载, 尺寸:', icon.size)
  const defaultName = `${baseName.value}_${icon.width}x${icon.height}.png`

  try {
    const filePath = await save({
      defaultPath: defaultName,
      filters: [{ name: 'PNG', extensions: ['png'] }],
    })
    if (!filePath) {
      console.log('[IconGrid] 用户取消了保存')
      return
    }

    console.log('[IconGrid] 保存至:', filePath)
    await invoke('save_icon_file', {
      dataUrl: icon.data_url,
      filePath: filePath,
    })
    console.log('[IconGrid] 保存成功')
    ElMessage.success(`图标已保存至 ${filePath}`)
  } catch (e: any) {
    console.error('[IconGrid] 保存失败:', e)
    ElMessage.error(`保存失败：${e}`)
  }
}

</script>

<template>
  <div class="icon-grid-wrapper glass-panel">
    <div class="grid-header">
      <h3 class="grid-title">已提取的图标</h3>
      <span class="grid-hint">左键选中 · 右键下载</span>
    </div>

    <div v-if="icons.length === 0" class="grid-empty">
      <p>没有可显示的图标</p>
    </div>

    <div v-else class="icon-grid">
      <div
        v-for="icon in icons"
        :key="icon.size"
        class="icon-card glass-card"
        :class="{
          'is-selected': selectedIcon?.size === icon.size
        }"
        @click="selectIcon(icon)"
        @contextmenu="onContextMenu($event, icon)"
      >
        <div v-if="selectedIcon?.size === icon.size" class="selected-badge">
          <Check :size="14" />
        </div>

        <div class="icon-preview">
          <img :src="icon.data_url" :alt="`${icon.size}x${icon.size}`" />
        </div>

        <div class="icon-info">
          <span class="icon-size">{{ icon.width }}×{{ icon.height }}</span>
          <span class="icon-dim">px</span>
        </div>
      </div>
    </div>

    <!-- Right-click context menu -->
    <Teleport to="body">
      <div
        v-if="contextMenu.visible"
        class="context-menu glass-panel"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="context-menu-item" @click="downloadDirect">
          <Download :size="16" />
          <span>下载</span>
          <span class="context-shortcut">PNG 原图</span>
        </div>

      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.icon-grid-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
  min-height: 0;
  overflow: hidden;
}

.grid-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.grid-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.grid-hint {
  font-size: 12px;
  color: var(--text-muted);
}

.grid-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: 14px;
}

.icon-grid {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
  gap: 12px;
  overflow-y: auto;
  padding-right: 4px;
  align-content: start;
}

.icon-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 16px 12px 14px;
  cursor: pointer;
  position: relative;
  transition: all var(--transition-smooth);
}

.icon-card:hover {
  transform: translateY(-2px);
}

.icon-card.is-selected {
  border-color: var(--accent);
  background: rgba(99, 179, 237, 0.08);
  box-shadow: 0 0 20px rgba(99, 179, 237, 0.15);
}

.selected-badge {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--gradient-2);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(79, 209, 197, 0.4);
  z-index: 2;
}

.icon-preview {
  width: 72px;
  height: 72px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.03);
  overflow: hidden;
  padding: 4px;
}

.icon-preview img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  image-rendering: auto;
}

.icon-info {
  display: flex;
  align-items: baseline;
  gap: 3px;
}

.icon-size {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: var(--font-mono);
  letter-spacing: -0.3px;
}

.icon-dim {
  font-size: 11px;
  color: var(--text-muted);
}

/* ---- Context Menu ---- */
.context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 200px;
  padding: 6px;
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  gap: 2px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
  animation: fadeInUp 0.15s ease-out;
}

.context-menu-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
  color: var(--text-primary);
  font-size: 14px;
  white-space: nowrap;
}

.context-menu-item:hover {
  background: var(--glass-bg-hover);
  color: var(--accent-light);
}

.context-shortcut {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-muted);
  opacity: 0.7;
}
</style>
