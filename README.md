# Ximage Image Compressor

A cross-platform desktop image compression app built with Tauri 2.x, Vue 3, TypeScript, and TailwindCSS.

> 中文版: `readme.md`

## Features

- ✅ Two-column layout (settings on the left, drop zone on the right)
- ✅ Supports JPG/JPEG, PNG, WEBP
- ✅ Lossy compression with manual quality sliders
- ✅ Lossless option (PNG/WebP, speed-optimized)
- ✅ PNG lossy quantization for better size reduction
- ✅ Drag & drop import (native file paths)
- ✅ Image preview with thumbnails
- ✅ Real-time progress (per-file completion)
- ✅ Remembers last settings and output path
- ✅ Cross-platform (macOS, Windows)

## Tech Stack

- **Frontend**: Vue 3 + TypeScript
- **UI**: TailwindCSS
- **Desktop**: Tauri 2.x
- **Package Manager**: pnpm
- **Image Processing**: Rust (`image-rs`, `imagequant`, `oxipng`, `libwebp`)

## Usage

1. Drag images into the right area or click to select files
2. Adjust compression settings
3. Click **Compress Images**
4. Review results and savings (if no output folder is set, files are overwritten)

## Compression Settings

- **Lossless**: Only PNG/WebP are lossless; JPEG remains lossy
- **JPG Quality**: Adjust JPEG quality (10%-100%)
- **WEBP Quality**: Adjust WebP quality (10%-100%)
- **PNG Quality**: Adjust PNG lossy quantization (10%-100%)

## Notes

- **No output folder**: will overwrite original files
- **Dragged files**: if original path is unavailable, you will be asked to select an output folder
- **PNG lossy**: uses palette quantization for smaller size with possible minor artifacts

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

## Project Structure

```
src/                 # Frontend source
├── App.vue         # Main UI
├── main.ts         # App entry
├── style.css       # Global styles
src-tauri/          # Tauri backend
├── Cargo.toml      # Rust deps
├── src/
│   └── main.rs     # Rust logic
├── tauri.conf.json # Tauri config
└── capabilities/   # Permissions
```

## License

MIT
