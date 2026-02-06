<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { readFile, stat } from '@tauri-apps/plugin-fs';
import { getCurrentWebview } from '@tauri-apps/api/webview';

interface CompressionSettings {
  lossless: boolean;
  qualityJpg: number;
  qualityWebp: number;
  qualityPng: number;
}

interface FileItem {
  name: string;
  path: string;
  size: number;
  format: string;
  status: 'pending' | 'processing' | 'success' | 'error';
  compressedSize: number;
  compressionRatio: number;
  url?: string;
}

const settings = ref<CompressionSettings>({
  lossless: false,
  qualityJpg: 50,
  qualityWebp: 65,
  qualityPng: 80,
});

const files = ref<FileItem[]>([]);
const isCompressing = ref(false);
const totalProgress = ref(0);
const isDragging = ref(false);
const outputPath = ref<string>('');

const settingsStorageKey = 'x-image:settings';
const outputPathStorageKey = 'x-image:output-path';

const getMimeType = (fileName: string): string => {
  const lowerExt = fileName.split('.').pop()?.toLowerCase() || '';
  if (lowerExt === 'png') return 'image/png';
  if (lowerExt === 'jpg' || lowerExt === 'jpeg') return 'image/jpeg';
  if (lowerExt === 'webp') return 'image/webp';
  return '';
};

const addFileByPath = async (filePath: string) => {
  const fileName = filePath.split('/').pop() || 'unknown';
  const format = fileName.split('.').pop()?.toUpperCase() || 'UNKNOWN';
  if (!['PNG', 'JPG', 'JPEG', 'WEBP'].includes(format)) return;
  if (files.value.some(file => file.path === filePath)) return;

  try {
    const metadata = await stat(filePath);
    const bytes = await readFile(filePath);
    const blob = new Blob([bytes], { type: getMimeType(fileName) });
    const url = URL.createObjectURL(blob);

    files.value.push({
      name: fileName,
      path: filePath,
      size: metadata.size,
      format,
      status: 'pending',
      compressedSize: 0,
      compressionRatio: 0,
      url
    });
  } catch (e) {
    console.error('读取文件失败:', filePath, e);
  }
};

const buildFileData = async (file: FileItem, index: number) => {
  if (file.path) {
    const bytes = await readFile(file.path);
    const base64Data = btoa(
      Array.from(bytes as Uint8Array, (byte) => String.fromCharCode(byte)).join('')
    );
    return {
      name: file.name,
      data: base64Data,
      format: file.format.toLowerCase(),
      sourceIndex: index,
      sourcePath: file.path || undefined,
    };
  }

  if (file.url && file.url.startsWith('blob:')) {
    const response = await fetch(file.url);
    const blob = await response.blob();
    const base64Data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result as string;
        resolve(result.split(',')[1] || '');
      };
      reader.onerror = reject;
      reader.readAsDataURL(blob);
    });

    return {
      name: file.name,
      data: base64Data,
      format: file.format.toLowerCase(),
      sourceIndex: index,
      sourcePath: file.path || undefined,
    };
  }

  return null;
};

let unlistenDragDrop: null | (() => void) = null;
onMounted(async () => {
  try {
    const storedSettings = localStorage.getItem(settingsStorageKey);
    if (storedSettings) {
      const parsed = JSON.parse(storedSettings) as Partial<CompressionSettings>;
      settings.value = { ...settings.value, ...parsed };
    }
    const storedOutputPath = localStorage.getItem(outputPathStorageKey);
    if (storedOutputPath) {
      outputPath.value = storedOutputPath;
    }

    unlistenDragDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
      const payload = 'payload' in event ? event.payload : event;
      if (payload.type === 'enter' || payload.type === 'over') {
        isDragging.value = true;
        return;
      }
      if (payload.type === 'leave') {
        isDragging.value = false;
        return;
      }
      if (payload.type === 'drop') {
        isDragging.value = false;
        if ('paths' in payload && payload.paths?.length) {
          for (const path of payload.paths) {
            await addFileByPath(path);
          }
        }
      }
    });
  } catch (e) {
    console.error('注册拖拽事件失败:', e);
  }
});

watch(
  settings,
  (value) => {
    localStorage.setItem(settingsStorageKey, JSON.stringify(value));
  },
  { deep: true }
);

watch(outputPath, (value) => {
  if (value) {
    localStorage.setItem(outputPathStorageKey, value);
  } else {
    localStorage.removeItem(outputPathStorageKey);
  }
});

onBeforeUnmount(() => {
  if (unlistenDragDrop) {
    unlistenDragDrop();
    unlistenDragDrop = null;
  }
});

const selectOutputPath = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择压缩后图片的保存位置'
    });
    if (selected) {
      outputPath.value = selected as string;
    }
  } catch (error) {
    console.error('选择保存路径失败:', error);
  }
};

const selectFiles = async () => {
  try {
    const selected = await open({
      directory: false,
      multiple: true,
      title: '选择图片文件',
      filters: [{
        name: '图片文件',
        extensions: ['png', 'jpg', 'jpeg', 'webp']
      }]
    });
    
    if (selected && Array.isArray(selected)) {
      for (const filePath of selected) {
        await addFileByPath(filePath);
      }
    }
  } catch (error) {
    console.error('选择文件失败:', error);
  }
};

const totalSize = computed(() => {
  const totalBytes = files.value.reduce((sum, file) => sum + file.size, 0);
  return formatSize(totalBytes);
});

const totalSavingsPercent = computed(() => {
  const original = files.value.reduce((sum, file) => sum + file.size, 0);
  const compressed = files.value.reduce((sum, file) => sum + (file.compressedSize || 0), 0);
  if (original === 0) return '0%';
  const percent = Math.round(((original - compressed) / original) * 100);
  return `${percent}%`;
});

const formatSize = (size: number): string => {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(2)} KB`;
  return `${(size / (1024 * 1024)).toFixed(2)} MB`;
};

const getStatusText = (status: string): string => {
  switch (status) {
    case 'pending': return '等待中';
    case 'processing': return '压缩中';
    case 'success': return '成功';
    case 'error': return '失败';
    default: return status;
  }
};

const onDragOver = (e: DragEvent) => {
  e.preventDefault();
  e.stopPropagation();
  isDragging.value = true;
};

const onDragLeave = (e: DragEvent) => {
  e.preventDefault();
  e.stopPropagation();
  isDragging.value = false;
};

const onDrop = (e: DragEvent) => {
  e.preventDefault();
  e.stopPropagation();
  isDragging.value = false;

  if (e.dataTransfer?.items && e.dataTransfer.items.length > 0) {
    Array.from(e.dataTransfer.items).forEach((item) => {
      if (item.kind === 'file') {
        const file = item.getAsFile();
        if (file) processFile(file);
      }
    });
  } else if (e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
    Array.from(e.dataTransfer.files).forEach(file => {
      processFile(file);
    });
  }
};

const processFile = (file: File) => {
  const isImage = file.type.startsWith('image/') || /\.(png|jpg|jpeg|webp)$/i.test(file.name);
  
  if (isImage) {
    const format = file.name.split('.').pop()?.toUpperCase() || 'UNKNOWN';
    const url = URL.createObjectURL(file);
    
    files.value.push({
      name: file.name,
      path: '',
      size: file.size,
      format,
      status: 'pending',
      compressedSize: 0,
      compressionRatio: 0,
      url
    });
  }
};

const compressImages = async () => {
  if (files.value.length === 0) return;

  if (!outputPath.value) {
    const hasMissingPath = files.value.some(file => !file.path);
    if (hasMissingPath) {
      alert('拖拽添加的文件无法覆盖原文件，请选择保存位置或使用“选择文件”导入。');
      return;
    }
  }

  isCompressing.value = true;
  totalProgress.value = 0;

  files.value.forEach(file => {
    file.status = 'processing';
  });

  try {
    let processedCount = 0;
    for (let index = 0; index < files.value.length; index += 1) {
      const file = files.value[index];
      try {
        const fileData = await buildFileData(file, index);
        if (!fileData) {
          file.status = 'error';
        } else {
            const results = await invoke<[string, number, number, string, number][]>('compress_uploaded_files', {
              fileData: [fileData],
              lossless: settings.value.lossless,
              qualityJpg: settings.value.qualityJpg,
              qualityWebp: settings.value.qualityWebp,
              qualityPng: settings.value.qualityPng,
              preserveExif: true,
              resizeWidth: undefined,
              resizeHeight: undefined,
              maintainAspectRatio: undefined,
              outputPath: outputPath.value || undefined,
            });

          const result = results[0];
          if (result) {
            const [, originalSize, compressedSize, status] = result;
            if (status === 'success') {
              file.status = 'success';
              file.compressedSize = compressedSize;
              file.compressionRatio = Math.round((1 - compressedSize / originalSize) * 100);
            } else {
              file.status = 'error';
            }
          } else {
            file.status = 'error';
          }
        }
      } catch (e) {
        console.error(`压缩 ${file.name} 失败:`, e);
        file.status = 'error';
      } finally {
        processedCount += 1;
        totalProgress.value = Math.round((processedCount / files.value.length) * 100);
      }
    }
  } catch (error) {
    console.error('压缩失败:', error);
  } finally {
    isCompressing.value = false;
    if (files.value.length > 0) {
      totalProgress.value = 100;
    }
  }
};

const clearFiles = () => {
  files.value.forEach(file => {
    if (file.url) URL.revokeObjectURL(file.url);
  });
  files.value = [];
};
</script>

<template>
  <div class="app-wrapper">
    <!-- Header -->
    <header class="app-header">
      <div class="app-title">
        <img class="app-icon" src="/src-tauri/icons/icon.png" alt="Ximage" />
        <span>Ximage 图片压缩工具</span>
      </div>
    </header>

    <!-- Main Content -->
    <div class="content-area">
      <!-- Left Panel - Settings -->
      <aside class="settings-panel">
        <div class="panel-content">
          <div class="settings-section">
            <div class="section-title">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
              <h3>压缩设置</h3>
            </div>

            <div class="setting-group">
              <label class="checkbox-label">
                <input type="checkbox" v-model="settings.lossless" />
                <span>无损压缩</span>
              </label>
              <p class="hint">保持原始画质，文件大小可能较大</p>
            </div>

            <div class="setting-group" v-if="!settings.lossless">
              <label class="input-label">JPG 质量: {{ settings.qualityJpg }}%</label>
              <input type="range" v-model.number="settings.qualityJpg" min="10" max="100" class="slider" />
              <div class="range-labels">
                <span>低</span>
                <span>高</span>
              </div>
            </div>

            <div class="setting-group" v-if="!settings.lossless">
              <label class="input-label">WEBP 质量: {{ settings.qualityWebp }}%</label>
              <input type="range" v-model.number="settings.qualityWebp" min="10" max="100" class="slider" />
              <div class="range-labels">
                <span>低</span>
                <span>高</span>
              </div>
            </div>

            <div class="setting-group" v-if="!settings.lossless">
              <label class="input-label">PNG 质量: {{ settings.qualityPng }}%</label>
              <input type="range" v-model.number="settings.qualityPng" min="10" max="100" class="slider" />
              <div class="range-labels">
                <span>低</span>
                <span>高</span>
              </div>
            </div>

            <div class="setting-group">
              <label class="input-label">保存位置</label>
              <div class="path-row">
                <input type="text" :value="outputPath || '默认位置（原文件所在目录）'" readonly />
                <button @click="selectOutputPath">选择...</button>
              </div>
              <p class="hint">{{ outputPath ? '压缩后的图片将保存到选择的文件夹' : '未选择时将覆盖原文件' }}</p>
            </div>

            <div class="button-row">
              <button class="btn-compress" :disabled="files.length === 0 || isCompressing" @click="compressImages">
                {{ isCompressing ? "压缩中..." : "压缩图片" }}
              </button>
            </div>
          </div>

          <div class="stats-section">
            <h3>统计信息</h3>
            <div class="stats-row">
              <div class="stat-box">
                <div class="stat-number">{{ files.length }}</div>
                <div class="stat-text">文件数量</div>
              </div>
              <div class="stat-box">
                <div class="stat-number">{{ totalSize }}</div>
                <div class="stat-text">总大小</div>
              </div>
              <div class="stat-box">
                <div class="stat-number">{{ totalSavingsPercent }}</div>
                <div class="stat-text">优化</div>
              </div>
            </div>
          </div>
        </div>
      </aside>

      <!-- Right Panel - Drop Zone -->
      <main class="drop-panel">
        <div
          class="drop-area"
          :class="{ 'is-dragging': isDragging }"
          @dragover="onDragOver"
          @dragleave="onDragLeave"
          @drop="onDrop"
        >
          <div v-if="files.length === 0" class="empty-area" @click="selectFiles">
            <svg class="upload-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            <h2>拖拽图片到这里</h2>
            <p>支持 PNG、JPG、WebP 格式</p>
            <button class="btn-file" @click.stop="selectFiles">选择文件</button>
          </div>

          <div v-else class="files-area">
            <div class="files-toolbar">
              <h3>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="7 10 12 15 17 10" />
                  <line x1="12" y1="15" x2="12" y2="3" />
                </svg>
                文件列表 ({{ files.length }})
              </h3>
              <div class="toolbar-actions">
                <button class="btn-add" @click="selectFiles" :disabled="isCompressing">添加文件</button>
                <button class="btn-clear-list" @click="clearFiles" :disabled="files.length === 0 || isCompressing">清空</button>
              </div>
            </div>

            <div v-if="isCompressing" class="progress-container">
              <div class="progress-header">
                <span>总进度</span>
                <span>{{ totalProgress }}%</span>
              </div>
              <div class="progress-bar">
                <div class="progress-fill" :style="{ width: `${totalProgress}%` }"></div>
              </div>
            </div>

            <div class="files-table-container">
              <table class="files-table">
                <thead>
                  <tr>
                    <th class="col-thumb">预览</th>
                    <th class="col-name">文件名</th>
                    <th class="col-format">格式</th>
                    <th class="col-size">大小</th>
                    <th class="col-status">状态</th>
                    <th class="col-result">结果</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(file, index) in files" :key="index">
                    <td class="col-thumb">
                      <img v-if="file.url" :src="file.url" :alt="file.name" />
                      <div v-else class="thumb-placeholder"></div>
                    </td>
                    <td class="col-name" :title="file.name">{{ file.name }}</td>
                    <td class="col-format">{{ file.format }}</td>
                    <td class="col-size">{{ formatSize(file.size) }}</td>
                    <td class="col-status" :class="file.status">{{ getStatusText(file.status) }}</td>
                    <td class="col-result">
                      <template v-if="file.compressedSize > 0">
                        <span>{{ formatSize(file.compressedSize) }}</span>
                        <span class="savings">↓{{ file.compressionRatio }}%</span>
                      </template>
                      <span v-else class="no-result">-</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </main>
    </div>
  </div>
</template>

<style>
/* Reset & Base */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  width: 100%;
  height: 100%;
  overflow: hidden;
}

/* App Layout */
.app-wrapper {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #f5f7fa;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
}

/* Header */
.app-header {
  height: 56px;
  min-height: 56px;
  background: #fff;
  border-bottom: 1px solid #e4e7ed;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  box-shadow: 0 1px 4px rgba(0,0,0,0.05);
}


.app-title {
  font-size: 18px;
  font-weight: 600;
  color: #409eff;
  display: flex;
  align-items: center;
  gap: 10px;
}

.app-icon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
}

/* Content Area */
.content-area {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

/* Settings Panel */
.settings-panel {
  width: 320px;
  min-width: 280px;
  max-width: 380px;
  background: #fff;
  border-right: 1px solid #e4e7ed;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 0;
  padding: 0;
}

.panel-content {
  flex: 1;
  overflow-y: auto;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0;
}

.settings-section {
  background: #f5f7fa;
  border-radius: 0;
  padding: 16px;
  border-bottom: 1px solid #e4e7ed;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid #e4e7ed;
}

.section-title svg {
  width: 18px;
  height: 18px;
  color: #409eff;
}

.section-title h3 {
  font-size: 15px;
  font-weight: 600;
  color: #303133;
}

.setting-group {
  margin-bottom: 16px;
}

.setting-group:last-child {
  margin-bottom: 0;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  font-size: 14px;
  color: #606266;
}

.checkbox-label input[type="checkbox"] {
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.hint {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
  margin-left: 22px;
}

.input-label {
  display: block;
  font-size: 14px;
  color: #606266;
  margin-bottom: 8px;
}


.resize-options {
  background: #fff;
  border-radius: 6px;
  padding: 12px;
  margin: -8px 0 16px 22px;
}

.dimension-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-bottom: 12px;
}

.dimension-input label {
  display: block;
  font-size: 12px;
  color: #909399;
  margin-bottom: 4px;
}

.dimension-input input {
  width: 100%;
  padding: 6px 10px;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  font-size: 13px;
}

.path-row {
  display: flex;
  gap: 8px;
}

.path-row input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  font-size: 13px;
  background: #f5f7fa;
  color: #606266;
}

.path-row button {
  padding: 8px 16px;
  background: #fff;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  font-size: 13px;
  color: #606266;
  cursor: pointer;
  white-space: nowrap;
}

.path-row button:hover {
  border-color: #409eff;
  color: #409eff;
}

.button-row {
  display: flex;
  gap: 10px;
  margin-top: 20px;
  padding-top: 16px;
  border-top: 1px solid #e4e7ed;
}

.btn-compress {
  flex: 1;
  padding: 10px 20px;
  background: #409eff;
  color: #fff;
  border: none;
  border-radius: 4px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.3s;
}

.btn-compress:hover:not(:disabled) {
  background: #66b1ff;
}

.btn-compress:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}


.stats-section {
  background: #ecf5ff;
  border-top: 1px solid #d9ecff;
  border-bottom: 1px solid #d9ecff;
  border-radius: 0;
  padding: 16px;
  margin-top: auto;
}

.stats-section h3 {
  font-size: 14px;
  color: #409eff;
  margin-bottom: 12px;
}

.stats-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 12px;
}

.stat-box {
  text-align: center;
}

.stat-number {
  font-size: 24px;
  font-weight: 600;
  color: #409eff;
  line-height: 1.2;
}

.stat-text {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
}

/* Drop Panel */
.drop-panel {
  flex: 1;
  padding: 0;
  min-width: 0;
  overflow: hidden;
}

.drop-area {
  width: 100%;
  height: 100%;
  background: #fff;
  border-radius: 8px;
  border: 2px dashed transparent;
  overflow: hidden;
  transition: all 0.3s;
}

.drop-area.is-dragging {
  border-color: #409eff;
  background: #f5f7fa;
}

.empty-area {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  cursor: pointer;
}

.empty-area:hover {
  border-color: #409eff;
}

.upload-icon {
  width: 64px;
  height: 64px;
  color: #c0c4cc;
}

.empty-area h2 {
  font-size: 20px;
  color: #606266;
  font-weight: 500;
}

.empty-area p {
  font-size: 14px;
  color: #909399;
}

.btn-file {
  padding: 10px 24px;
  background: #409eff;
  color: #fff;
  border: none;
  border-radius: 4px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.3s;
}

.btn-file:hover {
  background: #66b1ff;
}

.files-area {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 16px;
}

.files-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  flex-shrink: 0;
  padding: 0 4px;
}

.files-toolbar h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  color: #303133;
  font-weight: 500;
}

.files-toolbar svg {
  width: 18px;
  height: 18px;
}

.btn-add {
  padding: 6px 16px;
  background: #409eff;
  color: #fff;
  border: none;
  border-radius: 4px;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.3s;
}

.btn-add:hover:not(:disabled) {
  background: #66b1ff;
}

.btn-add:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.slider {
  width: 100%;
  height: 4px;
  -webkit-appearance: none;
  appearance: none;
  background: #dcdfe6;
  border-radius: 2px;
  outline: none;
}

.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: #409eff;
  border-radius: 50%;
  cursor: pointer;
}

.range-labels {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
}


.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-clear-list {
  padding: 6px 14px;
  background: #fff;
  color: #606266;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.3s;
}

.btn-clear-list:hover:not(:disabled) {
  border-color: #409eff;
  color: #409eff;
}

.btn-clear-list:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.progress-container {
  background: #f5f7fa;
  padding: 12px 16px;
  border-radius: 4px;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.progress-header {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  color: #606266;
  margin-bottom: 8px;
}

.progress-bar {
  height: 6px;
  background: #e4e7ed;
  border-radius: 3px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: #409eff;
  border-radius: 3px;
  transition: width 0.3s ease;
}

.files-table-container {
  flex: 1;
  overflow: auto;
  background: #fff;
  border-radius: 4px;
  border: 1px solid #e4e7ed;
}

.files-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.files-table th {
  background: #f5f7fa;
  padding: 12px 16px;
  text-align: left;
  font-weight: 500;
  color: #606266;
  border-bottom: 1px solid #e4e7ed;
  white-space: nowrap;
  position: sticky;
  top: 0;
  z-index: 1;
}

.files-table td {
  padding: 12px 16px;
  border-bottom: 1px solid #f0f0f0;
  color: #606266;
}

.files-table tbody tr:hover {
  background: #f5f7fa;
}

.col-thumb {
  width: 60px;
  text-align: center;
}

.col-thumb img {
  width: 40px;
  height: 40px;
  object-fit: cover;
  border-radius: 4px;
}

.thumb-placeholder {
  width: 40px;
  height: 40px;
  background: #e4e7ed;
  border-radius: 4px;
  margin: 0 auto;
}

.col-name {
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.col-format {
  width: 80px;
  text-align: center;
}

.col-size {
  width: 100px;
  text-align: right;
}

.col-status {
  width: 80px;
  text-align: center;
  font-weight: 500;
}

.col-status.pending { color: #909399; }
.col-status.processing { color: #409eff; }
.col-status.success { color: #67c23a; }
.col-status.error { color: #f56c6c; }

.col-result {
  width: 120px;
  text-align: right;
}

.col-result .savings {
  color: #67c23a;
  font-weight: 600;
  margin-left: 8px;
}

.col-result .no-result {
  color: #c0c4cc;
}

/* Scrollbar */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: #f5f7fa;
}

::-webkit-scrollbar-thumb {
  background: #c0c4cc;
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: #909399;
}
</style>
