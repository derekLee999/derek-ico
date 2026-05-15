<script setup lang="ts">
import { open } from '@tauri-apps/plugin-dialog'
import { UploadFilled } from '@element-plus/icons-vue'

defineProps<{
  isLoading: boolean
}>()

const emit = defineEmits<{
  'file-dropped': [path: string]
}>()

async function onClickDropZone() {
  const selected = await open({
    multiple: false,
    filters: [{
      name: '可执行文件 / 快捷方式',
      extensions: ['exe', 'dll', 'lnk', 'ico'],
    }],
  })

  console.log('[DropZone] open 返回:', selected, typeof selected)

  if (selected) {
    // Tauri v2 dialog may return string or { path: string }
    const filePath = typeof selected === 'string' ? selected : (selected as any).path || (selected as any)
    if (filePath) {
      console.log('[DropZone] 文件路径:', filePath)
      emit('file-dropped', filePath)
    }
  }
}
</script>

<template>
  <div class="drop-zone glass-panel" @click="onClickDropZone">
    <div class="drop-content">
      <div class="drop-icon-wrapper">
        <div class="drop-icon-ring"></div>
        <UploadFilled :size="48" class="drop-icon" />
      </div>
      <h2 class="drop-title">点击选择或拖放文件</h2>
      <p class="drop-desc">
        点击此处选择文件，或将 <code>.exe</code>、<code>.dll</code>、<code>.lnk</code> 文件拖入窗口
      </p>
      <div class="drop-formats">
        <el-tag size="small" effect="dark" round>EXE</el-tag>
        <el-tag size="small" effect="dark" round>DLL</el-tag>
        <el-tag size="small" effect="dark" round>LNK</el-tag>
        <el-tag size="small" effect="dark" round>ICO</el-tag>
      </div>
    </div>
  </div>
</template>

<style scoped>
.drop-zone {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  overflow: hidden;
  cursor: pointer;
  transition: all var(--transition-smooth);
  min-height: 320px;
}

.drop-zone:hover {
  border-color: var(--glass-border-hover);
  box-shadow: var(--shadow-md), 0 0 30px rgba(99, 179, 237, 0.08);
}

.drop-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  z-index: 1;
}

.drop-icon-wrapper {
  position: relative;
  width: 96px;
  height: 96px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.drop-icon-ring {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  border: 2px dashed var(--glass-border);
  animation: spin 20s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.drop-icon {
  color: var(--text-muted);
  opacity: 0.6;
  z-index: 1;
  transition: all var(--transition-smooth);
}

.drop-zone:hover .drop-icon {
  color: var(--accent-light);
  opacity: 0.9;
}

.drop-title {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.3px;
}

.drop-desc {
  font-size: 14px;
  color: var(--text-muted);
  text-align: center;
  line-height: 1.7;
}

.drop-desc code {
  font-family: var(--font-mono);
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid var(--glass-border);
  color: var(--accent-light);
}

.drop-formats {
  display: flex;
  gap: 8px;
  margin-top: 2px;
}
</style>
