#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::anyhow;
use base64::Engine;
use image::ImageFormat;
use image::GenericImageView;
use img_parts::ImageEXIF;
use oxipng::StripChunks;
use imagequant::RGBA as QuantRgba;
use std::collections::HashMap;
use std::sync::Mutex;
use std::{fs, path::Path};
use tauri::{Manager, Result};
use uuid::Uuid;

// Global temporary directory tracker to clean up resources
lazy_static::lazy_static! {
    static ref TEMP_DIRS: Mutex<HashMap<String, std::time::SystemTime>> = Mutex::new(HashMap::new());
}

struct CompressionConfig {
    lossless: bool,
    quality_jpg: u8,
    quality_webp: u8,
    quality_png: u8,
    preserve_exif: bool,
    resize_width: Option<u32>,
    resize_height: Option<u32>,
}

// Sanitize filename to prevent path traversal attacks
fn sanitize_filename(filename: &str) -> String {
    // Remove any path separators and parent directory references
    let sanitized = filename
        .replace("..", "")
        .replace("/", "_")
        .replace("\\", "_")
        .replace("\0", ""); // Prevent null byte injection

    // Only allow alphanumeric characters, dots, hyphens, and underscores
    let mut clean = String::new();
    for c in sanitized.chars() {
        if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
            clean.push(c);
        } else {
            clean.push('_');
        }
    }

    clean
}

fn detect_image_format(path: &Path) -> anyhow::Result<ImageFormat> {
    let ext = path
        .extension()
        .ok_or(anyhow!("No extension"))?
        .to_string_lossy()
        .to_lowercase();

    match ext.as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "webp" => Ok(ImageFormat::WebP),
        _ => Err(anyhow!("Unsupported format: {}", ext)),
    }
}

fn preserve_exif_data(original_path: &Path, compressed_path: &Path, format: ImageFormat) -> anyhow::Result<()> {
    let original_bytes = fs::read(original_path)?;
    let compressed_bytes = fs::read(compressed_path)?;
    
    let exif_data: Option<img_parts::Bytes> = match format {
        ImageFormat::Jpeg => {
            let jpeg = img_parts::jpeg::Jpeg::from_bytes(original_bytes.into())?;
            jpeg.exif()
        }
        _ => None,
    };
    
    if let Some(exif) = exif_data {
        let new_bytes = match format {
            ImageFormat::Jpeg => {
                let mut jpeg = img_parts::jpeg::Jpeg::from_bytes(compressed_bytes.into())?;
                jpeg.set_exif(Some(exif));
                let mut output = Vec::new();
                jpeg.encoder().write_to(&mut output)?;
                output
            }
            _ => compressed_bytes,
        };
        fs::write(compressed_path, new_bytes)?;
    }
    
    Ok(())
}

fn compress_image(
    original_path: &Path,
    config: &CompressionConfig,
    output_path: Option<&Path>,
    maintain_aspect_ratio: bool,
) -> anyhow::Result<(u64, u64)> {
    let format = detect_image_format(original_path)?;
    let original_size = fs::metadata(original_path)?.len();

    let img = image::open(original_path)?;
    let mut processed_img = img;

    // Resize if needed
    if let (Some(width), Some(height)) = (config.resize_width, config.resize_height) {
        if maintain_aspect_ratio {
            let (orig_w, orig_h) = processed_img.dimensions();
            let scale_w = width as f64 / orig_w as f64;
            let scale_h = height as f64 / orig_h as f64;
            let scale = scale_w.min(scale_h);
            let new_w = (orig_w as f64 * scale).round().max(1.0) as u32;
            let new_h = (orig_h as f64 * scale).round().max(1.0) as u32;
            processed_img = processed_img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);
        } else {
            processed_img = processed_img.resize(width, height, image::imageops::FilterType::Lanczos3);
        }
    }

    // Create a secure temporary directory for this operation
    let temp_dir = std::env::temp_dir()
        .join("Ximage-compress")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&temp_dir).map_err(|e| anyhow!("Failed to create temp dir: {}", e))?;

    // Track the temp directory for cleanup
    {
        let mut dirs = TEMP_DIRS.lock().unwrap();
        dirs.insert(
            temp_dir.to_string_lossy().to_string(),
            std::time::SystemTime::now(),
        );
    }

    // Generate a unique temporary file name
    let file_stem = original_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("temp");
    let extension = original_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("tmp");

    let temp_path = temp_dir.join(format!("{}_compressed.{}", file_stem, extension));

    match format {
        ImageFormat::Png => {
            if config.lossless {
                processed_img.save_with_format(&temp_path, ImageFormat::Png)?;
                let png_data = fs::read(&temp_path)?;
                // Faster lossless optimization preset
                let optimized = oxipng::optimize_from_memory(&png_data, &oxipng::Options::from_preset(1))?;
                fs::write(&temp_path, optimized)?;
            } else {
                let rgba = processed_img.to_rgba8();
                let (width, height) = rgba.dimensions();
                let pixels: Vec<QuantRgba> = rgba
                    .as_raw()
                    .chunks_exact(4)
                    .map(|c| QuantRgba::new(c[0], c[1], c[2], c[3]))
                    .collect();

                let quantize_once = |target: u8, min_offset: u8, speed: u8, dither: f32| -> anyhow::Result<Vec<u8>> {
                    let mut attr = imagequant::Attributes::new();
                    let min = target.saturating_sub(min_offset);
                    attr.set_quality(min, target)?;
                    // Balanced speed/quality
                    attr.set_speed(speed.into())?;

                    let mut img = attr.new_image(pixels.clone(), width as usize, height as usize, 0.0)?;
                    let mut res = attr.quantize(&mut img)?;
                    res.set_dithering_level(dither)?;
                    let (palette, indexed_pixels) = res.remapped(&mut img)?;

                    let mut palette_bytes = Vec::with_capacity(palette.len() * 3);
                    let mut trns = Vec::with_capacity(palette.len());
                    for color in palette {
                        palette_bytes.extend_from_slice(&[color.r, color.g, color.b]);
                        trns.push(color.a);
                    }

                    let mut output = Vec::new();
                    {
                        let cursor = std::io::Cursor::new(&mut output);
                        let mut encoder = png::Encoder::new(cursor, width, height);
                        encoder.set_color(png::ColorType::Indexed);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder.set_palette(palette_bytes);
                        if trns.iter().any(|&a| a < 255) {
                            encoder.set_trns(trns);
                        }
                        let mut writer = encoder.write_header()?;
                        writer.write_image_data(&indexed_pixels)?;
                    }
                    Ok(output)
                };

                let target = config.quality_png.min(100).max(10);
                let (min_offset, speed, dither) = if target >= 80 {
                    (25, 8, 0.6)
                } else if target >= 60 {
                    (30, 9, 0.8)
                } else {
                    (40, 10, 1.0)
                };
                let selected = quantize_once(target, min_offset, speed, dither)?;

                fs::write(&temp_path, selected)?;
                let png_data = fs::read(&temp_path)?;
                let mut options = oxipng::Options::from_preset(2);
                options.strip = StripChunks::All;
                if let Ok(optimized) = oxipng::optimize_from_memory(&png_data, &options) {
                    let _ = fs::write(&temp_path, optimized);
                }
            }
        }
        ImageFormat::Jpeg => {
            if config.lossless {
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    std::io::BufWriter::new(std::fs::File::create(&temp_path)?),
                    100,
                );
                encoder.encode_image(&processed_img)?;
            } else {
                let encode_jpeg = |quality: u8| -> anyhow::Result<Vec<u8>> {
                    let mut buffer = Vec::new();
                    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
                    encoder.encode_image(&processed_img)?;
                    Ok(buffer)
                };

                let base_q = config.quality_jpg.clamp(60, 95);
                let mut candidates = vec![
                    base_q.saturating_add(10),
                    base_q.saturating_add(5),
                    base_q,
                    base_q.saturating_sub(5),
                    base_q.saturating_sub(10),
                ];
                candidates.retain(|q| *q >= 60 && *q <= 95);
                candidates.sort_unstable();
                candidates.dedup();

                let base = encode_jpeg(base_q)?;
                let base_size = base.len().max(1);
                let mut best = (base, 0.0);

                for cand in candidates {
                    let data = encode_jpeg(cand)?;
                    let size = data.len().max(1);
                    let size_improve = (base_size as f64 - size as f64) / base_size as f64;
                    let quality_score = cand as f64 / 100.0;
                    let speed_penalty = if cand >= 85 { 0.05 } else if cand >= 75 { 0.07 } else { 0.1 };
                    let score = quality_score * 0.5 + size_improve * 0.4 - speed_penalty * 0.1;
                    if score > best.1 {
                        best = (data, score);
                    }
                }

                fs::write(&temp_path, best.0)?;
            }
        }
        ImageFormat::WebP => {
            if config.lossless {
                processed_img.save_with_format(&temp_path, ImageFormat::WebP)?;
            } else {
                let rgba = processed_img.to_rgba8();
                let (width, height) = rgba.dimensions();
                let encoder = webp::Encoder::from_rgba(&rgba, width, height);

                let mut webp_config = webp::WebPConfig::new().unwrap();
                webp_config.quality = config.quality_webp as f32;
                webp_config.method = 4;
                webp_config.sns_strength = 70;
                webp_config.filter_strength = 30;
                webp_config.filter_sharpness = 3;
                webp_config.autofilter = 1;
                webp_config.alpha_quality = 80;
                webp_config.alpha_compression = 1;
                webp_config.near_lossless = 60;
                webp_config.exact = 0;
                webp_config.thread_level = 1;

                let webp_data = encoder
                    .encode_advanced(&webp_config)
                    .map_err(|e| anyhow!("WebP encode failed: {:?}", e))?;
                fs::write(&temp_path, &*webp_data)?;
            }
        }
        _ => return Err(anyhow!("Unsupported format")),
    }

    if config.preserve_exif && format == ImageFormat::Jpeg {
        let _ = preserve_exif_data(original_path, &temp_path, format);
    }

    let final_path = if let Some(out_path) = output_path {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        out_path.to_path_buf()
    } else {
        original_path.to_path_buf()
    };

    fs::copy(&temp_path, &final_path)?;

    let compressed_size = fs::metadata(&final_path)?.len();

    std::fs::remove_dir_all(temp_dir).ok();

    Ok((original_size, compressed_size))
}

#[tauri::command]
async fn compress_images(
    paths: Vec<String>,
    lossless: bool,
    quality_jpg: u8,
    quality_webp: u8,
    quality_png: u8,
    preserve_exif: bool,
    resize_width: Option<u32>,
    resize_height: Option<u32>,
) -> Result<Vec<(String, u64, u64, String)>> {
    let config = CompressionConfig {
        lossless,
        quality_jpg,
        quality_webp,
        quality_png,
        preserve_exif,
        resize_width,
        resize_height,
    };

    let mut results = Vec::new();

    for path_str in paths {
        let path = Path::new(&path_str);

        match compress_image(path, &config, None, false) {
            Ok((original_size, compressed_size)) => {
                results.push((
                    path_str,
                    original_size,
                    compressed_size,
                    "success".to_string(),
                ));
            }
            Err(e) => {
                results.push((path_str, 0, 0, format!("error: {}", e)));
            }
        }
    }

    Ok(results)
}

#[tauri::command]
async fn stat_path(path: String) -> std::result::Result<u64, String> {
    match std::fs::metadata(&path) {
        Ok(meta) => Ok(meta.len()),
        Err(e) => Err(e.to_string()),
    }
}

// 新增：处理前端上传的 base64 编码文件数据
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileData {
    name: String,
    data: String, // base64 编码
    format: String,
    source_index: u32,
    source_path: Option<String>,
}

#[tauri::command]
async fn compress_uploaded_files(
    file_data: Vec<FileData>,
    lossless: bool,
    quality_jpg: u8,
    quality_webp: u8,
    quality_png: u8,
    preserve_exif: bool,
    resize_width: Option<u32>,
    resize_height: Option<u32>,
    maintain_aspect_ratio: Option<bool>,
    output_path: Option<String>,
) -> std::result::Result<Vec<(String, u64, u64, String, u32)>, String> {
    println!("🎯 后端收到前端上传的 {} 个文件进行压缩", file_data.len());
    let mut results = Vec::new();

    let keep_aspect_ratio = maintain_aspect_ratio.unwrap_or(false);

    // Create a secure temporary directory for this operation
    let temp_dir = std::env::temp_dir()
        .join("Ximage-upload")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

    for file_info in file_data {
        println!("📂 处理文件: {}", file_info.name);

        // Sanitize the filename to prevent path traversal
        let sanitized_filename = sanitize_filename(&file_info.name);

        // Validate file format
        let valid_formats = ["png", "jpg", "jpeg", "webp"];
        if !valid_formats
            .iter()
            .any(|&f| f.eq_ignore_ascii_case(&file_info.format))
        {
            println!("❌ 不支持的文件格式: {}", file_info.format);
            results.push((file_info.name, 0, 0, "unsupported_format".to_string(), file_info.source_index));
            continue;
        }

        // 解码 base64 数据
        let file_bytes = match base64::engine::general_purpose::STANDARD.decode(&file_info.data) {
            Ok(bytes) => {
                println!("✅ Base64 解码成功, {} 字节", bytes.len());

                // Check file size (e.g., limit to 50MB)
                if bytes.len() > 50 * 1024 * 1024 {
                    println!("❌ 文件过大 (>50MB): {} bytes", bytes.len());
                    results.push((file_info.name, 0, 0, "file_too_large".to_string(), file_info.source_index));
                    continue;
                }

                bytes
            }
            Err(e) => {
                println!("❌ Base64 解码失败: {}", e);
                results.push((file_info.name, 0, 0, "decode_failed".to_string(), file_info.source_index));
                continue;
            }
        };

        // 保存到临时文件
        let temp_path = temp_dir.join(&sanitized_filename);
        if let Err(e) = fs::write(&temp_path, &file_bytes) {
            println!("❌ 保存临时文件失败: {}", e);
            results.push((file_info.name, 0, 0, "save_failed".to_string(), file_info.source_index));
            continue;
        }

        println!("💾 临时文件已保存: {}", temp_path.display());

        // 压缩文件
        let config = CompressionConfig {
            lossless,
            quality_jpg,
            quality_webp,
            quality_png,
            preserve_exif,
            resize_width,
            resize_height,
        };

        let output_file_path = output_path.as_ref().map(|p| {
            let output_dir = Path::new(p);
            output_dir.join(&sanitized_filename)
        });

        let source_path = file_info.source_path.as_ref().map(Path::new);

        if output_file_path.is_none() && source_path.is_none() {
            println!("❌ 未提供原始路径，无法覆盖原文件");
            results.push((file_info.name, 0, 0, "missing_source_path".to_string(), file_info.source_index));
            continue;
        }

        let original_path = source_path.unwrap_or(temp_path.as_path());
        
        match compress_image(original_path, &config, output_file_path.as_deref(), keep_aspect_ratio) {
            Ok((original_size, compressed_size)) => {
                let ratio = if original_size > 0 {
                    let saved = original_size.saturating_sub(compressed_size);
                    ((saved as f64 / original_size as f64) * 100.0) as u32
                } else {
                    0
                };
                println!(
                    "✅ 压缩成功: {} -> {} (节省 {}%)",
                    original_size, compressed_size, ratio
                );
                results.push((
                    file_info.name,
                    original_size,
                    compressed_size,
                    "success".to_string(),
                    file_info.source_index,
                ));
            }
            Err(e) => {
                println!("❌ 压缩失败: {}", e);
                results.push((file_info.name, 0, 0, "compress_failed".to_string(), file_info.source_index));
            }
        }
    }

    // Clean up temp directory after processing
    std::fs::remove_dir_all(&temp_dir).ok(); // Ignore errors during cleanup

    println!("📦 后端返回压缩结果: {:?}", results);
    Ok(results)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                app.get_webview_window("main").unwrap().open_devtools();
            }
            Ok(())
        })
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            compress_images,
            stat_path,
            compress_uploaded_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
