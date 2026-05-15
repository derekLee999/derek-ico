<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { ElMessage } from 'element-plus'
import { Search, Download, Link } from '@element-plus/icons-vue'

interface FaviconData {
  size: number
  width: number
  height: number
  data_url: string
  source: string
}

const urlInput = ref('')
const isLoading = ref(false)
const favicons = ref<FaviconData[]>([])
const activeDomain = ref('')
const selectedIcon = ref<FaviconData | null>(null)
const contextMenu = ref({ visible: false, x: 0, y: 0, icon: null as FaviconData | null })

const hasIcons = computed(() => favicons.value.length > 0)

// Close menu on window click
if (typeof window !== 'undefined') {
  window.addEventListener('click', () => {
    contextMenu.value.visible = false
  })
}

async function fetchIcons() {
  const url = urlInput.value.trim()
  if (!url) {
    ElMessage.warning('请输入网址')
    return
  }

  isLoading.value = true
  favicons.value = []
  selectedIcon.value = null

  try {
    console.log('[SiteExtract] 获取图标, 网址:', url)
    const result = await invoke<FaviconData[]>('fetch_favicons', { url })
    console.log('[SiteExtract] 获取成功:', result.length, '个图标')
    favicons.value = result
    if (result.length > 0) {
      selectedIcon.value = result[0]

      // Extract domain for display
      try {
        const u = new URL(url.startsWith('http') ? url : 'https://' + url)
        activeDomain.value = u.hostname
      } catch {
        activeDomain.value = url
      }

      ElMessage.success(`从 ${activeDomain.value} 获取了 ${result.length} 个图标`)
    } else {
      ElMessage.warning('未找到图标')
    }
  } catch (e: any) {
    console.error('[SiteExtract] 获取失败:', e)
    ElMessage.error(`获取失败：${e}`)
  } finally {
    isLoading.value = false
  }
}

function selectIcon(icon: FaviconData) {
  selectedIcon.value = icon
}

function onContextMenu(e: MouseEvent, icon: FaviconData) {
  e.preventDefault()
  selectIcon(icon)
  contextMenu.value = {
    visible: true,
    x: e.clientX,
    y: e.clientY,
    icon,
  }
}

async function downloadIcon() {
  const icon = contextMenu.value.icon
  if (!icon) return
  contextMenu.value.visible = false

  const name = activeDomain.value || 'favicon'
  const defaultName = `${name}_${icon.width}x${icon.height}.png`

  try {
    const filePath = await save({
      defaultPath: defaultName,
      filters: [{ name: 'PNG', extensions: ['png'] }],
    })
    if (!filePath) return

    await invoke('save_icon_file', {
      dataUrl: icon.data_url,
      filePath,
    })
    ElMessage.success(`图标已保存至 ${filePath}`)
  } catch (e: any) {
    ElMessage.error(`保存失败：${e}`)
  }
}

// Handle Enter key
function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') fetchIcons()
}
</script>

<template>
  <div class="site-extract">
    <!-- URL Input Bar -->
    <div class="url-bar glass-card">
      <div class="url-input-wrapper">
        <el-icon :size="18" class="url-icon"><Link /></el-icon>
        <input
          v-model="urlInput"
          type="text"
          class="url-field"
          placeholder="输入网址，如 example.com 或 https://example.com"
          @keydown="onKeydown"
          :disabled="isLoading"
        />
      </div>
      <el-button
        type="primary"
        :icon="Search"
        round
        size="large"
        :loading="isLoading"
        @click="fetchIcons"
      >
        获取图标
      </el-button>
    </div>

    <!-- Loading -->
    <div v-if="isLoading" class="loading-state glass-panel fade-in-up">
      <div class="loading-spinner">
        <div class="spinner-ring"></div>
        <Search :size="36" class="spinner-icon" />
      </div>
      <p class="loading-text">正在获取图标...</p>
      <p class="loading-file">{{ activeDomain || urlInput }}</p>
    </div>

    <!-- Empty state -->
    <div v-if="!hasIcons && !isLoading" class="empty-state glass-panel fade-in-up">
      <div class="empty-icon-wrapper">
        <div class="empty-icon-ring"></div>
        <Link :size="48" class="empty-icon" />
      </div>
      <h2 class="empty-title">输入网址获取图标</h2>
      <p class="empty-desc">
        支持任意网站，自动通过多种方式查找 favicon 图标
      </p>
      <div class="empty-tips">
        <el-tag size="small" effect="dark" round>HTML 解析</el-tag>
        <el-tag size="small" effect="dark" round>/favicon.ico</el-tag>
        <el-tag size="small" effect="dark" round>Google 服务</el-tag>
      </div>
    </div>

    <!-- Results -->
    <div v-if="hasIcons" class="results-view fade-in-up">
      <div class="file-info-bar glass-card">
        <div class="file-info-left">
          <el-tag type="primary" effect="dark" round>{{ activeDomain }}</el-tag>
          <span class="icon-count">找到 {{ favicons.length }} 个图标（右键下载）</span>
        </div>
      </div>

      <div class="favicon-grid-wrapper glass-panel">
        <div class="favicon-grid">
          <div
            v-for="icon in favicons"
            :key="icon.size + icon.source"
            class="icon-card glass-card"
            :class="{ 'is-selected': selectedIcon?.size === icon.size }"
            @click="selectIcon(icon)"
            @contextmenu="onContextMenu($event, icon)"
          >
            <div class="icon-preview">
              <img :src="icon.data_url" :alt="`${icon.size}x${icon.size}`" />
            </div>
            <div class="icon-info">
              <span class="icon-size">{{ icon.width }}×{{ icon.height }}</span>
              <span class="icon-dim">px</span>
            </div>
            <div class="icon-source">{{ icon.source }}</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Context Menu -->
    <Teleport to="body">
      <div
        v-if="contextMenu.visible"
        class="context-menu glass-panel"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="context-menu-item" @click="downloadIcon">
          <Download :size="16" />
          <span>下载</span>
          <span class="context-shortcut">PNG</span>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.site-extract {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
}

.url-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  flex-shrink: 0;
}

.url-input-wrapper {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  background: rgba(255, 255, 255, 0.04);
  border-radius: var(--radius-sm);
  padding: 0 14px;
}

.url-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.url-field {
  flex: 1;
  background: none;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 15px;
  padding: 12px 0;
  font-family: var(--font-sans);
}

.url-field::placeholder {
  color: var(--text-muted);
}

/* Loading */
.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  padding: 80px 40px;
  max-width: 420px;
  margin: auto;
  flex: 1;
}

.loading-spinner {
  position: relative;
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.spinner-ring {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  border: 3px solid transparent;
  border-top-color: var(--accent);
  border-right-color: var(--accent-light);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.spinner-icon {
  color: var(--accent);
  opacity: 0.7;
}

.loading-text {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
}

.loading-file {
  font-size: 13px;
  color: var(--text-muted);
  font-family: var(--font-mono);
}

/* Empty */
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
  padding: 80px 40px;
}

.empty-icon-wrapper {
  position: relative;
  width: 96px;
  height: 96px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.empty-icon-ring {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  border: 2px dashed var(--glass-border);
  animation: spin 20s linear infinite;
}

.empty-icon {
  color: var(--text-muted);
  opacity: 0.6;
  z-index: 1;
}

.empty-title {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.3px;
}

.empty-desc {
  font-size: 14px;
  color: var(--text-muted);
  text-align: center;
  line-height: 1.7;
}

.empty-tips {
  display: flex;
  gap: 8px;
  margin-top: 2px;
}

/* Results */
.results-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
}

.file-info-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  flex-shrink: 0;
}

.file-info-left {
  display: flex;
  align-items: center;
  gap: 14px;
}

.icon-count {
  font-size: 13px;
  color: var(--text-secondary);
}

.favicon-grid-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
  min-height: 0;
  overflow: hidden;
}

.favicon-grid {
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
  gap: 8px;
  padding: 16px 12px 12px;
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

.icon-source {
  font-size: 10px;
  color: var(--text-muted);
  opacity: 0.5;
  font-family: var(--font-mono);
}

/* Context menu */
.context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 180px;
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
