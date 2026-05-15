<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ElMessage } from 'element-plus'
import { Picture, Monitor, Link, ArrowLeft } from '@element-plus/icons-vue'
import DropZone from './components/DropZone.vue'
import IconGrid from './components/IconGrid.vue'
import SiteExtract from './components/SiteExtract.vue'

interface IconData {
  size: number
  width: number
  height: number
  data_url: string
}

// ---- Tabs ----
const activeTab = ref<'software' | 'website'>('software')

// ---- Software extraction state ----
const isLoading = ref(false)
const icons = ref<IconData[]>([])
const selectedIcon = ref<IconData | null>(null)
const activeFileName = ref('')

const hasIcons = computed(() => icons.value.length > 0)

let unlisten: UnlistenFn | null = null

onMounted(async () => {
  console.log('[App] 组件挂载，注册 file-dropped 事件监听')
  unlisten = await listen<string>('file-dropped', (event) => {
    console.log('[App] 收到 file-dropped 事件:', event.payload)
    onFileDropped(event.payload)
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
})

async function onFileDropped(filePath: string) {
  console.log('[App] onFileDropped 开始处理:', filePath)
  isLoading.value = true
  icons.value = []
  selectedIcon.value = null

  const parts = filePath.replace(/\\/g, '/').split('/')
  activeFileName.value = parts[parts.length - 1] || filePath

  try {
    console.log('[App] 调用 extract_icons 命令, 文件:', filePath)
    const result = await invoke<IconData[]>('extract_icons', { path: filePath })
    console.log('[App] extract_icons 返回:', result.length, '个图标')
    icons.value = result
    if (result.length > 0) {
      selectedIcon.value = result[0]
      console.log('[App] 默认选中图标尺寸:', result[0].size)
      ElMessage.success(`从 ${activeFileName.value} 中提取了 ${result.length} 个图标尺寸`)
    } else {
      console.warn('[App] 未找到图标')
      ElMessage.warning('未在该文件中找到图标')
    }
  } catch (e: any) {
    console.error('[App] 提取图标失败:', e)
    ElMessage.error(`图标提取失败：${e}`)
  } finally {
    isLoading.value = false
  }
}

function selectIcon(icon: IconData) {
  selectedIcon.value = icon
}

function resetState() {
  icons.value = []
  selectedIcon.value = null
  activeFileName.value = ''
}
</script>

<template>
  <div class="app-root">
    <div class="bg-orb orb-1"></div>
    <div class="bg-orb orb-2"></div>
    <div class="bg-orb orb-3"></div>

    <div class="app-container">
      <!-- Tab Bar -->
      <div class="tab-bar glass-panel">
        <button
          class="tab-item"
          :class="{ active: activeTab === 'software' }"
          @click="activeTab = 'software'"
        >
          <Monitor :size="14" style="width: 90px; height: 90px;" />
          <span>软件提取</span>
        </button>
        <button
          class="tab-item"
          :class="{ active: activeTab === 'website' }"
          @click="activeTab = 'website'"
        >
          <Link :size="14" style="width: 100px; height: 100px;" />
          <span>网站提取</span>
        </button>
      </div>

      <!-- Tab Content -->
      <main class="app-main">
        <!-- 软件提取 Tab -->
        <template v-if="activeTab === 'software'">
          <!-- 引导区 -->
          <DropZone
            v-if="!hasIcons && !isLoading"
            :is-loading="isLoading"
            @file-dropped="onFileDropped"
            class="fade-in-up"
          />

          <!-- 加载中 -->
          <div v-if="isLoading" class="loading-state glass-panel fade-in-up">
            <div class="loading-spinner">
              <div class="spinner-ring"></div>
              <Picture :size="36" class="spinner-icon" />
            </div>
            <p class="loading-text">正在提取图标...</p>
            <p class="loading-file">{{ activeFileName }}</p>
          </div>

          <!-- 结果 -->
          <div v-if="hasIcons" class="results-view fade-in-up">
            <div class="file-info-bar glass-card">
              <div class="file-info-left">
                <el-tag type="primary" effect="dark" round>
                  {{ activeFileName }}
                </el-tag>
                <span class="icon-count">找到 {{ icons.length }} 个图标尺寸（右键下载）</span>
              </div>
              <el-button :icon="ArrowLeft" round size="small" @click="resetState">返回</el-button>
            </div>
            <IconGrid
              :icons="icons"
              :selected-icon="selectedIcon"
              :file-name="activeFileName"
              @select="selectIcon"
            />
          </div>
        </template>

        <!-- 网站提取 Tab -->
        <SiteExtract v-if="activeTab === 'website'" />
      </main>

      <footer class="app-footer">
        <span class="footer-text">拖入 .exe / .lnk 提取图标 · 输入网址获取 favicon · 右键下载</span>
        <span class="footer-version">v0.2.0</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.app-root {
  width: 100%;
  height: 100%;
  position: relative;
  overflow: hidden;
}

.bg-orb {
  position: absolute;
  border-radius: 50%;
  filter: blur(120px);
  opacity: 0.15;
  pointer-events: none;
  z-index: 0;
}

.orb-1 {
  width: 600px;
  height: 600px;
  background: radial-gradient(circle, #63b3ed 0%, transparent 70%);
  top: -200px;
  right: -150px;
  animation: float 8s ease-in-out infinite;
}

.orb-2 {
  width: 500px;
  height: 500px;
  background: radial-gradient(circle, #b794f4 0%, transparent 70%);
  bottom: -150px;
  left: -100px;
  animation: float 10s ease-in-out infinite reverse;
}

.orb-3 {
  width: 400px;
  height: 400px;
  background: radial-gradient(circle, #f687b3 0%, transparent 70%);
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  animation: float 12s ease-in-out infinite 2s;
}

.app-container {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 10px 28px 20px;
}

/* Tabs */
.tab-bar {
  display: flex;
  gap: 3px;
  padding: 3px;
  border-radius: var(--radius-sm);
  flex-shrink: 0;
  margin-bottom: 6px;
}

.tab-item {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 3px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-smooth);
  font-family: var(--font-sans);
  line-height: 1;
}

.tab-item:hover {
  color: var(--text-secondary);
  background: var(--glass-bg);
}

.tab-item.active {
  color: var(--text-primary);
  background: var(--glass-bg-active);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.2);
}

.app-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

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

.app-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 0 4px;
  flex-shrink: 0;
}

.footer-text {
  font-size: 12px;
  color: var(--text-muted);
}

.footer-version {
  font-size: 11px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  opacity: 0.5;
}
</style>
