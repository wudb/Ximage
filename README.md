# Ximage Image Compressor

Language: **English** | [中文](README_CN.md)

A cross-platform desktop image compression app built with Tauri 2.x, Vue 3, TypeScript, and TailwindCSS.

## Release

- Current version: `0.0.2`
- macOS DMG: `Ximage-0.0.2.dmg`

## Features

- ✅ Two-column layout (settings on the left, drop zone on the right)
- ✅ Supports JPG/JPEG, PNG, WEBP
- ✅ Lossy compression with manual quality sliders
- ✅ Lossless option (PNG/WebP; JPEG is still lossy)
- ✅ PNG lossy quantization for better size reduction
- ✅ Drag & drop import + file picker
- ✅ Image preview with thumbnails and per-file status
- ✅ Overall progress bar
- ✅ Remembers last settings, language, theme, and output path
- ✅ System/Light/Dark theme
- ✅ Bilingual UI (中文 / English)
- ✅ Cross-platform (macOS, Windows)

## Tech Stack

- **Frontend**: Vue 3 + TypeScript
- **UI**: TailwindCSS
- **Desktop**: Tauri 2.x
- **Package Manager**: pnpm
- **Image Processing**: Rust (`image`, `imagequant`, `oxipng`, `webp`, `png`, `img-parts`)

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
- **EXIF**: JPEG EXIF is preserved when possible

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
