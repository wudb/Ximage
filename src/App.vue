<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { readFile, stat } from '@tauri-apps/plugin-fs';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';

interface CompressionSettings {
  lossless: boolean;
  qualityJpg: number;
  qualityWebp: number;
  qualityPng: number;
  theme: 'system' | 'light' | 'dark';
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
  theme: 'system',
});

const files = ref<FileItem[]>([]);
const isCompressing = ref(false);
const totalProgress = ref(0);
const isDragging = ref(false);
const outputPath = ref<string>('');

const settingsStorageKey = 'Ximage:settings';
const outputPathStorageKey = 'Ximage:output-path';
const localeKey = 'Ximage:locale';

type Locale = 'zh' | 'en';
const locale = ref<Locale>('en');

const detectLocale = (): Locale => {
  const lang = (navigator.language || '').toLowerCase();
  if (lang.startsWith('zh') || lang.includes('-cn')) return 'zh';
  return 'en';
};

const i18n: Record<Locale, Record<string, string>> = {
  zh: {
    appName: 'Ximage 图片压缩工具',
    settings: '压缩设置',
    lossless: '无损压缩',
    losslessHint: '保持原始画质，文件大小可能较大',
    jpgQuality: 'JPG 质量',
    webpQuality: 'WEBP 质量',
    pngQuality: 'PNG 质量',
    savePath: '保存位置',
    saveDefault: '默认位置（原文件所在目录）',
    savePicked: '压缩后的图片将保存到选择的文件夹',
    saveOverwrite: '未选择时将覆盖原文件',
    choose: '选择...',
    compress: '压缩图片',
    compressing: '压缩中...',
    stats: '统计信息',
    filesCount: '文件数量',
    totalSize: '总大小',
    optimize: '优化',
    dropHere: '拖拽图片到这里',
    supported: '支持 PNG、JPG、WebP 格式',
    chooseFiles: '选择文件',
    fileList: '文件列表',
    addFiles: '添加文件',
    clear: '清空',
    totalProgress: '总进度',
    preview: '预览',
    fileName: '文件名',
    format: '格式',
    size: '大小',
    status: '状态',
    result: '结果',
    statusPending: '等待中',
    statusProcessing: '压缩中',
    statusSuccess: '成功',
    statusError: '失败',
    theme: '主题',
    themeSystem: '跟随系统',
    themeLight: '明亮',
    themeDark: '暗黑',
    dragNoPath: '拖拽添加的文件无法覆盖原文件，请选择保存位置或使用“选择文件”导入。',
    selectOutputTitle: '选择压缩后图片的保存位置',
    selectFileTitle: '选择图片文件',
    imageFiles: '图片文件',
    low: '低',
    high: '高',
  },
  en: {
    appName: 'Ximage Image Compressor',
    settings: 'Compression Settings',
    lossless: 'Lossless',
    losslessHint: 'Preserve quality; file size may be larger',
    jpgQuality: 'JPG Quality',
    webpQuality: 'WEBP Quality',
    pngQuality: 'PNG Quality',
    savePath: 'Save Location',
    saveDefault: 'Default (original folder)',
    savePicked: 'Compressed files will be saved to the selected folder',
    saveOverwrite: 'If not selected, original files will be overwritten',
    choose: 'Choose...',
    compress: 'Compress Images',
    compressing: 'Compressing...',
    stats: 'Statistics',
    filesCount: 'Files',
    totalSize: 'Total Size',
    optimize: 'Optimization',
    dropHere: 'Drop images here',
    supported: 'Supports PNG, JPG, WebP',
    chooseFiles: 'Select Files',
    fileList: 'Files',
    addFiles: 'Add Files',
    clear: 'Clear',
    totalProgress: 'Overall Progress',
    preview: 'Preview',
    fileName: 'Name',
    format: 'Format',
    size: 'Size',
    status: 'Status',
    result: 'Result',
    statusPending: 'Pending',
    statusProcessing: 'Processing',
    statusSuccess: 'Success',
    statusError: 'Error',
    theme: 'Theme',
    themeSystem: 'System',
    themeLight: 'Light',
    themeDark: 'Dark',
    dragNoPath: 'Dragged files cannot overwrite originals. Please choose a save location or use “Select Files”.',
    selectOutputTitle: 'Select output folder',
    selectFileTitle: 'Select image files',
    imageFiles: 'Images',
    low: 'Low',
    high: 'High',
  }
};

type I18nKey = keyof (typeof i18n)['en'];
const t = (key: I18nKey) => i18n[locale.value][key];

const systemThemeQuery = window.matchMedia('(prefers-color-scheme: dark)');
const handleSystemThemeChange = () => {
  if (settings.value.theme === 'system') {
    applyTheme('system');
  }
};
const applyTheme = (theme: CompressionSettings['theme']) => {
  const resolved = theme === 'system'
    ? (systemThemeQuery.matches ? 'dark' : 'light')
    : theme;
  document.documentElement.classList.toggle('theme-dark', resolved === 'dark');
  document.documentElement.classList.toggle('theme-light', resolved === 'light');
};

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
    const storedLocale = localStorage.getItem(localeKey);
    if (storedLocale === 'zh' || storedLocale === 'en') {
      locale.value = storedLocale;
    } else {
      locale.value = detectLocale();
      localStorage.setItem(localeKey, locale.value);
    }

    applyTheme(settings.value.theme);
    systemThemeQuery.addEventListener('change', handleSystemThemeChange);

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

watch(
  () => settings.value.theme,
  (value) => {
    applyTheme(value);
  }
);

watch(outputPath, (value) => {
  if (value) {
    localStorage.setItem(outputPathStorageKey, value);
  } else {
    localStorage.removeItem(outputPathStorageKey);
  }
});

watch(
  () => locale.value,
  () => {
    document.title = t('appName');
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  if (unlistenDragDrop) {
    unlistenDragDrop();
    unlistenDragDrop = null;
  }
  systemThemeQuery.removeEventListener('change', handleSystemThemeChange);
});


const selectOutputPath = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t('selectOutputTitle')
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
      title: t('selectFileTitle'),
      filters: [{
        name: t('imageFiles'),
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
    case 'pending': return t('statusPending');
    case 'processing': return t('statusProcessing');
    case 'success': return t('statusSuccess');
    case 'error': return t('statusError');
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

const onTitlebarMouseDown = async (event: MouseEvent) => {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  if (target && target.closest('button, input, select, textarea, a')) return;
  event.preventDefault();
  await getCurrentWindow().startDragging();
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
      alert(t('dragNoPath'));
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
    <div class="titlebar" @mousedown="onTitlebarMouseDown">
      <div class="titlebar-left"></div>
      <div class="titlebar-center">
        <span>{{ t('appName') }}</span>
      </div>
      <div class="titlebar-right"></div>
    </div>

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
              <h3>{{ t('settings') }}</h3>
            </div>

            <div class="setting-group">
              <label class="checkbox-label">
                <input type="checkbox" v-model="settings.lossless" />
                <span>{{ t('lossless') }}</span>
              </label>
              <p class="hint">{{ t('losslessHint') }}</p>
            </div>

            <div class="setting-group" v-if="!settings.lossless">
              <label class="input-label">{{ t('jpgQuality') }}: {{ settings.qualityJpg }}%</label>
              <input type="range" v-model.number="settings.qualityJpg" min="10" max="100" class="slider" />
              <div class="range-labels">
                <span>{{ t('low') }}</span>
                <span>{{ t('high') }}</span>
              </div>
            </div>

            <div class="setting-group" v-if="!settings.lossless">
              <label class="input-label">{{ t('webpQuality') }}: {{ settings.qualityWebp }}%</label>
              <input type="range" v-model.number="settings.qualityWebp" min="10" max="100" class="slider" />
              <div class="range-labels">
                <span>{{ t('low') }}</span>
                <span>{{ t('high') }}</span>
              </div>
            </div>

            <div class="setting-group" v-if="!settings.lossless">
              <label class="input-label">{{ t('pngQuality') }}: {{ settings.qualityPng }}%</label>
              <input type="range" v-model.number="settings.qualityPng" min="10" max="100" class="slider" />
              <div class="range-labels">
                <span>{{ t('low') }}</span>
                <span>{{ t('high') }}</span>
              </div>
            </div>

            <div class="setting-group">
              <label class="input-label">{{ t('savePath') }}</label>
              <div class="path-row">
                <input type="text" :value="outputPath || t('saveDefault')" readonly />
                <button @click="selectOutputPath">{{ t('choose') }}</button>
              </div>
              <p class="hint">{{ outputPath ? t('savePicked') : t('saveOverwrite') }}</p>
            </div>

            <div class="setting-group">
              <label class="input-label">{{ t('theme') }}</label>
              <div class="path-row">
                <select v-model="settings.theme">
                  <option value="system">{{ t('themeSystem') }}</option>
                  <option value="light">{{ t('themeLight') }}</option>
                  <option value="dark">{{ t('themeDark') }}</option>
                </select>
              </div>
            </div>

            <div class="button-row">
              <button class="btn-compress" :disabled="files.length === 0 || isCompressing" @click="compressImages">
                {{ isCompressing ? t('compressing') : t('compress') }}
              </button>
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
            <h2>{{ t('dropHere') }}</h2>
            <p>{{ t('supported') }}</p>
            <button class="btn-file" @click.stop="selectFiles">{{ t('chooseFiles') }}</button>
          </div>

          <div v-else class="files-area">
            <div class="files-toolbar">
              <h3>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="7 10 12 15 17 10" />
                  <line x1="12" y1="15" x2="12" y2="3" />
                </svg>
                {{ t('fileList') }} ({{ files.length }})
              </h3>
              <div class="toolbar-actions">
                <button class="btn-add" @click="selectFiles" :disabled="isCompressing">{{ t('addFiles') }}</button>
                <button class="btn-clear-list" @click="clearFiles" :disabled="files.length === 0 || isCompressing">{{ t('clear') }}</button>
              </div>
            </div>

            <div v-if="isCompressing" class="progress-container">
              <div class="progress-header">
                <span>{{ t('totalProgress') }}</span>
                <span>{{ totalProgress }}%</span>
              </div>
              <div class="progress-bar">
                <div class="progress-fill" :style="{ width: `${totalProgress}%` }"></div>
              </div>
            </div>

            <div class="files-table-container">
              <div class="files-table-scroll">
                <table class="files-table">
                  <thead>
                    <tr>
                      <th class="col-thumb">{{ t('preview') }}</th>
                      <th class="col-name">{{ t('fileName') }}</th>
                      <th class="col-format">{{ t('format') }}</th>
                      <th class="col-size">{{ t('size') }}</th>
                      <th class="col-status">{{ t('status') }}</th>
                      <th class="col-result">{{ t('result') }}</th>
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

:root,
.theme-light {
  --bg-app: #f4f5f9;
  --bg-panel: #ffffff;
  --bg-sidebar: #eef0f5;
  --bg-muted: #f7f8fb;
  --border: #e5e7ef;
  --border-strong: #d6dae6;
  --border-light: #eef0f5;
  --text-primary: #1f2430;
  --text-secondary: #4a5161;
  --text-muted: #7a8191;
  --text-disabled: #b2b7c3;
  --primary: #4f7cff;
  --primary-hover: #6a8dff;
  --primary-soft: #eef2ff;
  --primary-soft-border: #d9e0ff;
  --success: #48b870;
  --danger: #ff6b6b;
  --app-surface:
    radial-gradient(900px 380px at 82% -10%, color-mix(in srgb, var(--primary) 10%, transparent), transparent 60%),
    var(--bg-app);
}

.theme-dark {
  --bg-app: #101218;
  --bg-panel: #171a21;
  --bg-sidebar: #14171d;
  --bg-muted: #1e222b;
  --border: #2b2f39;
  --border-strong: #343945;
  --border-light: #232834;
  --text-primary: #eef0f4;
  --text-secondary: #c8ccd6;
  --text-muted: #9aa1ad;
  --text-disabled: #6b7280;
  --primary: #5b8dff;
  --primary-hover: #739dff;
  --primary-soft: #1c253a;
  --primary-soft-border: #2a3550;
  --success: #61d28f;
  --danger: #ff6b6b;
  --app-surface:
    radial-gradient(900px 380px at 82% -10%, color-mix(in srgb, var(--primary) 10%, transparent), transparent 60%),
    var(--bg-app);
}

/* App Layout */
.app-wrapper {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--app-surface);
  color: var(--text-primary);
  font-family: "Manrope", "SF Pro Display", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  letter-spacing: 0.1px;
}

/* Titlebar */
.titlebar {
  height: 56px;
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 0 18px;
  padding-top: 6px;
  padding-left: 88px; /* leave space for macOS traffic lights */
  padding-right: 64px;
  background: transparent;
  border-bottom: none;
  user-select: none;
  cursor: grab;
  -webkit-user-select: none;
}

.titlebar:active {
  cursor: grabbing;
}

.titlebar-left {
  height: 100%;
  min-width: 0;
}

.titlebar-center {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: 0.2px;
  padding: 0 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.titlebar-right {
  height: 100%;
  min-width: 0;
}

/* Content Area */
.content-area {
  flex: 1;
  display: grid;
  grid-template-columns: 300px 1fr;
  gap: 18px;
  overflow: hidden;
  min-height: 0;
  padding: 24px;
}

/* Settings Panel */
.settings-panel {
  width: 100%;
  min-width: 0;
  background: var(--bg-sidebar);
  border: 1px solid var(--border);
  border-radius: 16px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 0;
  padding: 0;
}

.panel-content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px;
}

.settings-section {
  background: var(--bg-panel);
  border-radius: 14px;
  padding: 16px;
  border: none;
  box-shadow: 0 8px 22px rgba(0,0,0,0.06);
}

.section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}

.section-title svg {
  width: 18px;
  height: 18px;
  color: var(--text-muted);
}

.section-title h3 {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
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
  gap: 10px;
  cursor: pointer;
  font-size: 14px;
  color: var(--text-secondary);
}

.checkbox-label input[type="checkbox"] {
  width: 14px;
  height: 14px;
  cursor: pointer;
}

.hint {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
  margin-left: 22px;
}

.input-label {
  display: block;
  font-size: 14px;
  color: var(--text-secondary);
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
  color: var(--text-muted);
  margin-bottom: 4px;
}

.dimension-input input {
  width: 100%;
  padding: 6px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  font-size: 13px;
  background: var(--bg-panel);
  color: var(--text-secondary);
}

.path-row {
  display: flex;
  gap: 8px;
  min-width: 0;
}

.path-row input {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  font-size: 13px;
  background: var(--bg-muted);
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path-row button {
  padding: 9px 16px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  font-size: 13px;
  color: var(--text-secondary);
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
}

.path-row button:hover {
  border-color: var(--primary);
  color: var(--primary);
}

.path-row select {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  font-size: 13px;
  background: var(--bg-panel);
  color: var(--text-secondary);
  appearance: none;
  -webkit-appearance: none;
  -moz-appearance: none;
  background-image:
    linear-gradient(45deg, transparent 50%, var(--text-muted) 50%),
    linear-gradient(135deg, var(--text-muted) 50%, transparent 50%),
    linear-gradient(to right, transparent, transparent);
  background-position:
    calc(100% - 18px) 50%,
    calc(100% - 12px) 50%,
    0 0;
  background-size:
    6px 6px,
    6px 6px,
    100% 100%;
  background-repeat: no-repeat;
  padding-right: 34px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.path-row select:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 25%, transparent);
}

.button-row {
  display: flex;
  gap: 10px;
  margin-top: 20px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}

.btn-compress {
  flex: 1;
  padding: 11px 20px;
  background: var(--primary);
  color: #fff;
  border: none;
  border-radius: 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 10px 24px color-mix(in srgb, var(--primary) 28%, transparent);
}

.btn-compress:hover:not(:disabled) {
  background: var(--primary-hover);
  transform: translateY(-1px);
}

.btn-compress:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}


.stats-section {
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 16px;
  margin-top: 0;
  box-shadow: 0 8px 22px rgba(0,0,0,0.06);
}

.stats-section h3 {
  font-size: 14px;
  color: var(--text-primary);
  margin-bottom: 12px;
}

.stats-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 12px;
}

.stat-box {
  text-align: center;
  padding: 10px 8px;
  border-radius: 12px;
  background: var(--bg-muted);
  border: 1px solid var(--border-light);
}

.stat-number {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.2;
}

.stat-text {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
}

/* Drop Panel */
.drop-panel {
  flex: 1;
  padding: 0;
  min-width: 0;
  overflow: hidden;
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: 18px;
  box-shadow: 0 12px 32px rgba(0,0,0,0.08);
}

.drop-area {
  width: 100%;
  height: 100%;
  background: var(--bg-panel);
  border-radius: 0;
  border: none;
  overflow: hidden;
  transition: all 0.2s ease;
}

.drop-area.is-dragging {
  border-color: var(--primary);
  background: var(--bg-muted);
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
  padding: 32px;
}

.upload-icon {
  width: 64px;
  height: 64px;
  color: var(--text-muted);
}

.empty-area h2 {
  font-size: 20px;
  color: var(--text-secondary);
  font-weight: 500;
}

.empty-area p {
  font-size: 14px;
  color: var(--text-muted);
}

.btn-file {
  padding: 10px 24px;
  background: var(--primary);
  color: #fff;
  border: none;
  border-radius: 12px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-file:hover {
  background: var(--primary-hover);
  transform: translateY(-1px);
}

.files-area {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 18px;
}

.files-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 14px;
  flex-shrink: 0;
  padding: 0 4px;
}

.files-toolbar h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  color: var(--text-primary);
  font-weight: 500;
}

.files-toolbar svg {
  width: 18px;
  height: 18px;
}

.btn-add {
  padding: 7px 16px;
  background: var(--primary);
  color: #fff;
  border: none;
  border-radius: 10px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-add:hover:not(:disabled) {
  background: var(--primary-hover);
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
  background: var(--border-strong);
  border-radius: 2px;
  outline: none;
}

.slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: var(--primary);
  border-radius: 50%;
  cursor: pointer;
  box-shadow: 0 4px 12px color-mix(in srgb, var(--primary) 35%, transparent);
}

.range-labels {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
}


.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-clear-list {
  padding: 7px 14px;
  background: var(--bg-panel);
  color: var(--text-secondary);
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-clear-list:hover:not(:disabled) {
  border-color: var(--primary);
  color: var(--primary);
}

.btn-clear-list:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.progress-container {
  background: var(--bg-muted);
  padding: 12px 16px;
  border-radius: 12px;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.progress-header {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  color: var(--text-secondary);
  margin-bottom: 8px;
}

.progress-bar {
  height: 6px;
  background: var(--border);
  border-radius: 3px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--primary);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.files-table-container {
  flex: 1;
  overflow: hidden;
  background: var(--bg-panel);
  border-radius: 12px;
  border: 1px solid var(--border);
}
.files-table-scroll {
  width: 100%;
  height: 100%;
  overflow: auto;
}

.files-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.files-table thead th:first-child {
  border-top-left-radius: 12px;
}

.files-table thead th:last-child {
  border-top-right-radius: 12px;
}

.files-table th {
  background: var(--bg-muted);
  padding: 12px 16px;
  text-align: left;
  font-weight: 500;
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border);
  white-space: nowrap;
  position: sticky;
  top: 0;
  z-index: 1;
}

.files-table td {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-light);
  color: var(--text-secondary);
}

.files-table tbody tr:hover {
  background: var(--bg-muted);
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
  background: var(--border);
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

.col-status.pending { color: var(--text-muted); }
.col-status.processing { color: var(--primary); }
.col-status.success { color: var(--success); }
.col-status.error { color: var(--danger); }

.col-result {
  width: 120px;
  text-align: right;
}

.col-result .savings {
  color: var(--success);
  font-weight: 600;
  margin-left: 8px;
}

.col-result .no-result {
  color: var(--text-disabled);
}

/* Scrollbar */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: var(--bg-muted);
}

::-webkit-scrollbar-thumb {
  background: var(--text-disabled);
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}
</style>
