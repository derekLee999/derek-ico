/// Windows icon extraction module using native Win32 APIs.
/// Extracts all icon sizes from .exe, .dll, .lnk, and .ico files.

use base64::Engine;
use image::ImageEncoder;
use serde::Serialize;
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct IconData {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    /// Base64-encoded PNG data
    pub data_url: String,
}

/// Main entry point: extract all icon sizes from a file path.
/// For .lnk shortcuts, resolves the target first to get the clean icon
/// (no arrow overlay) and to avoid Windows loader conflicts.
pub fn extract_all_icons(file_path: &str) -> Result<Vec<IconData>, String> {
    let path = Path::new(file_path);
    let is_lnk = path
        .extension()
        .map(|e| e.to_str().unwrap_or("").to_lowercase() == "lnk")
        .unwrap_or(false);

    if is_lnk {
        log::info!("[extract] 检测到快捷方式，解析目标路径...");
        extract_icons_from_shortcut(file_path)
    } else {
        extract_icons_from_file(file_path)
    }
}

/// Extract icons from a .lnk shortcut using COM IShellLink.
/// First checks if the shortcut has a custom icon set; if so, extracts from
/// the custom icon source. Otherwise, extracts from the resolved target path.
/// This avoids the arrow overlay that SHGetFileInfoW adds to .lnk files.
#[cfg(windows)]
fn extract_icons_from_shortcut(lnk_path: &str) -> Result<Vec<IconData>, String> {
    use windows::core::{Interface, PCWSTR, HSTRING};
    use windows::Win32::System::Com::{
        CoInitializeEx, CoCreateInstance, CoUninitialize,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        IPersistFile, STGM,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let mut com_initialized = false;

    unsafe {
        // Initialize COM
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_ok() {
            com_initialized = true;
        }
        // If it returns S_FALSE or RPC_E_CHANGED_MODE, COM is already initialized

        let result = (|| -> Result<Vec<IconData>, String> {
            // Create IShellLink instance
            let shell_link: IShellLinkW = CoCreateInstance(
                &ShellLink,
                None,
                CLSCTX_INPROC_SERVER,
            ).map_err(|e| format!("CoCreateInstance failed: {e:?}"))?;

            // Load the .lnk file via IPersistFile
            let persist_file: IPersistFile = shell_link.cast()
                .map_err(|e| format!("QueryInterface IPersistFile failed: {e:?}"))?;

            let wide_lnk: HSTRING = lnk_path.into();
            persist_file.Load(PCWSTR(wide_lnk.as_ptr()), STGM(0))
                .map_err(|e| format!("IPersistFile::Load failed: {e:?}"))?;

            // Try to get custom icon location from the shortcut
            let mut icon_path_buf = [0u16; 260];
            let mut icon_index: i32 = 0;
            let has_custom_icon = shell_link.GetIconLocation(
                &mut icon_path_buf,
                &mut icon_index,
            ).is_ok();

            let custom_icon_path = if has_custom_icon {
                let path_str = String::from_utf16_lossy(&icon_path_buf);
                let path_str = path_str.trim_end_matches('\0').to_string();
                if !path_str.is_empty() && Path::new(&path_str).exists() {
                    log::info!("[shortcut] 快捷方式自定义图标: {} (index={})", path_str, icon_index);
                    Some((path_str, icon_index))
                } else {
                    None
                }
            } else {
                None
            };

            // Get the resolved target path
            let mut target_buf = [0u16; 260];
            let target_path = if shell_link.GetPath(
                &mut target_buf,
                std::ptr::null_mut(),
                0,
            ).is_ok() {
                let path_str = String::from_utf16_lossy(&target_buf);
                let path_str = path_str.trim_end_matches('\0').to_string();
                if !path_str.is_empty() && Path::new(&path_str).exists() {
                    log::info!("[shortcut] 快捷方式目标路径: {}", path_str);
                    Some(path_str)
                } else {
                    None
                }
            } else {
                None
            };

            // Strategy:
            // 1. If shortcut has a custom icon, extract from that source
            // 2. Otherwise, extract from the target exe
            // 3. Fall back to extracting from the lnk itself (without SHGetFileInfo)
            if let Some((icon_source, _idx)) = custom_icon_path {
                log::info!("[shortcut] 使用自定义图标源提取");
                extract_icons_from_file(&icon_source)
            } else if let Some(target) = target_path {
                log::info!("[shortcut] 使用目标路径提取");
                extract_icons_from_file(&target)
            } else {
                log::warn!("[shortcut] 无法解析目标，回退到直接提取");
                extract_icons_from_pe_only(lnk_path)
            }
        })();

        if com_initialized {
            CoUninitialize();
        }

        result
    }
}

#[cfg(not(windows))]
fn extract_icons_from_shortcut(_lnk_path: &str) -> Result<Vec<IconData>, String> {
    Err("Shortcut resolution is only supported on Windows".to_string())
}

/// Extract icons from PE files only (ExtractIconExW + LoadImageW).
/// Does NOT use SHGetFileInfoW, so no arrow overlay.
#[cfg(windows)]
fn extract_icons_from_pe_only(file_path: &str) -> Result<Vec<IconData>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, HICON,
    };

    let wide_path: Vec<u16> = file_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut icons: Vec<IconData> = Vec::new();

    unsafe {
        let icon_count = ExtractIconExW(
            PCWSTR(wide_path.as_ptr()),
            -1,
            None,
            None,
            0,
        ) as usize;

        if icon_count > 0 {
            let mut large_icons: Vec<HICON> = vec![HICON(std::ptr::null_mut()); icon_count];
            let mut small_icons: Vec<HICON> = vec![HICON(std::ptr::null_mut()); icon_count];

            let extracted = ExtractIconExW(
                PCWSTR(wide_path.as_ptr()),
                0,
                Some(large_icons.as_mut_ptr()),
                Some(small_icons.as_mut_ptr()),
                icon_count as u32,
            ) as usize;

            for i in 0..extracted {
                if !large_icons[i].is_invalid() && large_icons[i].0 != std::ptr::null_mut() {
                    if let Ok(data) = hicon_to_png(large_icons[i], 0) {
                        if !icons.iter().any(|ic: &IconData| ic.size == data.size) {
                            icons.push(data);
                        }
                    }
                    let _ = DestroyIcon(large_icons[i]);
                }
                if !small_icons[i].is_invalid() && small_icons[i].0 != std::ptr::null_mut() {
                    if let Ok(data) = hicon_to_png(small_icons[i], 0) {
                        if !icons.iter().any(|ic: &IconData| ic.size == data.size) {
                            icons.push(data);
                        }
                    }
                    let _ = DestroyIcon(small_icons[i]);
                }
            }
        }
    }

    if icons.is_empty() {
        Err("No icons found in the file".to_string())
    } else {
        icons.sort_by_key(|i| i.size);
        Ok(icons)
    }
}

#[cfg(not(windows))]
fn extract_icons_from_pe_only(_path: &str) -> Result<Vec<IconData>, String> {
    Err("Icon extraction is only supported on Windows".to_string())
}

/// Extract all icon sizes from a file.
/// Uses ExtractIconExW + LoadImageW for PE files, and SHGetFileInfoW as fallback.
#[cfg(windows)]
fn extract_icons_from_file(file_path: &str) -> Result<Vec<IconData>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        ExtractIconExW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON,
        SHGFI_SMALLICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, HICON, LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR,
    };
    use windows::Win32::System::LibraryLoader::{
        LoadLibraryExW, LOAD_LIBRARY_AS_DATAFILE,
        LOAD_LIBRARY_AS_IMAGE_RESOURCE,
    };
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;

    let wide_path: Vec<u16> = file_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let path = Path::new(file_path);
    let is_pe = path.extension()
        .map(|e| {
            let ext = e.to_str().unwrap_or("").to_lowercase();
            ext == "exe" || ext == "dll" || ext == "ocx" || ext == "scr"
        })
        .unwrap_or(false);

    let mut icons: Vec<IconData> = Vec::new();

    unsafe {
        // --- Approach 1: ExtractIconExW (only for PE files) ---

        if is_pe {
            log::info!("[extract_file] 尝试 ExtractIconExW...");
            let icon_count = ExtractIconExW(
                PCWSTR(wide_path.as_ptr()),
                -1,
                None,
                None,
                0,
            ) as usize;

            log::info!("[extract_file] ExtractIconExW 报告 {} 个图标组", icon_count);

            if icon_count > 0 {
                // Extract one group at a time to be safe
                for group_idx in 0..icon_count {
                    let mut large_icon = HICON(std::ptr::null_mut());
                    let mut small_icon = HICON(std::ptr::null_mut());

                    let extracted = ExtractIconExW(
                        PCWSTR(wide_path.as_ptr()),
                        group_idx as i32,
                        Some(&mut large_icon),
                        Some(&mut small_icon),
                        1,
                    );

                    if extracted > 0 {
                        if !large_icon.is_invalid() && large_icon.0 != std::ptr::null_mut() {
                            match hicon_to_png(large_icon, 0) {
                                Ok(data) => {
                                    if !icons.iter().any(|ic| ic.size == data.size) {
                                        log::info!("[extract_file] ExtractIconExW large: {}x{}", data.width, data.height);
                                        icons.push(data);
                                    }
                                }
                                Err(e) => log::warn!("[extract_file] hicon_to_png (large) 失败: {}", e),
                            }
                            let _ = DestroyIcon(large_icon);
                        }
                        if !small_icon.is_invalid() && small_icon.0 != std::ptr::null_mut() {
                            match hicon_to_png(small_icon, 0) {
                                Ok(data) => {
                                    if !icons.iter().any(|ic| ic.size == data.size) {
                                        log::info!("[extract_file] ExtractIconExW small: {}x{}", data.width, data.height);
                                        icons.push(data);
                                    }
                                }
                                Err(e) => log::warn!("[extract_file] hicon_to_png (small) 失败: {}", e),
                            }
                            let _ = DestroyIcon(small_icon);
                        }
                    }
                }

            }
        }

        // --- Approach 2: LoadLibraryExW + LoadImageW (only for PE files) ---
        if is_pe {
            log::info!("[extract_file] 尝试 LoadLibraryExW + LoadImageW...");
            let h_module = LoadLibraryExW(
                PCWSTR(wide_path.as_ptr()),
                None,
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            );

            if let Ok(hm) = h_module {
                if !hm.is_invalid() && hm.0 != std::ptr::null_mut() {
                    let sizes = [16u32, 24, 32, 48, 64, 96, 128, 256];
                    // Use MAKEINTRESOURCE(1) — resource ID 1 is the conventional
                    // main icon in most PE files.
                    let resource_id: usize = 1;
                    let resource_ptr = PCWSTR(resource_id as *const u16);

                    for &size in &sizes {
                        let result = LoadImageW(
                            hm,
                            resource_ptr,
                            IMAGE_ICON,
                            size as i32,
                            size as i32,
                            LR_DEFAULTCOLOR,
                        );

                        if let Ok(handle) = result {
                            if !handle.is_invalid() && handle.0 != std::ptr::null_mut() {
                                let hicon = HICON(handle.0);
                                match hicon_to_png(hicon, size) {
                                    Ok(data) => {
                                        if !icons.iter().any(|ic| ic.size == data.size) {
                                            log::info!("[extract_file] LoadImageW: {}x{}", data.width, data.height);
                                            icons.push(data);
                                        }
                                    }
                                    Err(e) => log::warn!("[extract_file] hicon_to_png (LoadImageW {}px) 失败: {}", size, e),
                                }
                                let _ = DestroyIcon(hicon);
                            }
                        }
                    }
                    // Note: LOAD_LIBRARY_AS_DATAFILE handles are managed by the system.
                    // The handle will be cleaned up when the process exits.
                }
            }
        }

        // --- Approach 3: SHGetFileInfoW (fallback for non-PE files or if nothing found) ---
        if icons.is_empty() {
            log::info!("[extract_file] 尝试 SHGetFileInfoW...");

            // Large icon
            let mut sfi = SHFILEINFOW::default();
            let result = SHGetFileInfoW(
                PCWSTR(wide_path.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut sfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );

            if result != 0 && !sfi.hIcon.is_invalid() && sfi.hIcon.0 != std::ptr::null_mut() {
                match hicon_to_png(sfi.hIcon, 0) {
                    Ok(data) => {
                        if !icons.iter().any(|ic| ic.size == data.size) {
                            icons.push(data);
                        }
                    }
                    Err(e) => log::warn!("[extract_file] SHGetFileInfoW large 失败: {}", e),
                }
                let _ = DestroyIcon(sfi.hIcon);
            }

            // Small icon
            let mut sfi_small = SHFILEINFOW::default();
            let result_small = SHGetFileInfoW(
                PCWSTR(wide_path.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut sfi_small),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_SMALLICON,
            );

            if result_small != 0 && !sfi_small.hIcon.is_invalid() && sfi_small.hIcon.0 != std::ptr::null_mut() {
                match hicon_to_png(sfi_small.hIcon, 0) {
                    Ok(data) => {
                        if !icons.iter().any(|ic| ic.size == data.size) {
                            icons.push(data);
                        }
                    }
                    Err(e) => log::warn!("[extract_file] SHGetFileInfoW small 失败: {}", e),
                }
                let _ = DestroyIcon(sfi_small.hIcon);
            }
        }
    }

    if icons.is_empty() {
        Err("No icons found in the file".to_string())
    } else {
        icons.sort_by_key(|i| i.size);
        Ok(icons)
    }
}

#[cfg(not(windows))]
fn extract_icons_from_file(_path: &str) -> Result<Vec<IconData>, String> {
    Err("Icon extraction is only supported on Windows".to_string())
}

/// Convert an HICON to PNG data, returning an IconData struct.
/// Includes robust validation to prevent crashes from invalid/empty bitmaps.
#[cfg(windows)]
fn hicon_to_png(
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    _expected_size: u32,
) -> Result<IconData, String> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, RGBQUAD,
        SelectObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    unsafe {
        // Validate the icon handle first
        if hicon.is_invalid() || hicon.0.is_null() {
            return Err("Invalid HICON handle".to_string());
        }

        let mut icon_info = ICONINFO::default();
        GetIconInfo(hicon, &mut icon_info)
            .map_err(|e| format!("GetIconInfo failed: {e:?}"))?;

        let hbm_color = icon_info.hbmColor;
        let hbm_mask = icon_info.hbmMask;

        // Helper to cleanup bitmaps on error
        let cleanup_bitmaps = |color: windows::Win32::Graphics::Gdi::HBITMAP,
                                mask: windows::Win32::Graphics::Gdi::HBITMAP| {
            if !color.is_invalid() && !color.0.is_null() {
                let _ = DeleteObject(color);
            }
            if !mask.is_invalid() && !mask.0.is_null() {
                let _ = DeleteObject(mask);
            }
        };

        // Determine actual size from the bitmap
        let has_color = !hbm_color.is_invalid() && !hbm_color.0.is_null();

        let (src_bmp, actual_width, actual_height) = if has_color {
            let mut bmp = BITMAP::default();
            let ret = GetObjectW(
                hbm_color,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as *mut _),
            );
            if ret == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
                cleanup_bitmaps(hbm_color, hbm_mask);
                return Err("GetObjectW failed or returned invalid bitmap dimensions".to_string());
            }
            (hbm_color, bmp.bmWidth as u32, bmp.bmHeight as u32)
        } else {
            // Mask-only icon (monochrome). The mask is double height:
            // top half = AND mask, bottom half = XOR mask
            let mut bmp = BITMAP::default();
            let ret = GetObjectW(
                hbm_mask,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as *mut _),
            );
            if ret == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
                cleanup_bitmaps(hbm_color, hbm_mask);
                return Err("GetObjectW on mask bitmap failed".to_string());
            }
            // For monochrome icons, height is 2x the actual icon height
            let h = if bmp.bmHeight > bmp.bmWidth { bmp.bmWidth } else { bmp.bmHeight };
            (hbm_mask, bmp.bmWidth as u32, h as u32)
        };

        // Sanity check dimensions
        let width = actual_width;
        let height = actual_height;
        if width == 0 || height == 0 || width > 1024 || height > 1024 {
            cleanup_bitmaps(hbm_color, hbm_mask);
            return Err(format!("Invalid icon dimensions: {}x{}", width, height));
        }

        let pixel_count = (width as usize) * (height as usize) * 4;
        if pixel_count == 0 {
            cleanup_bitmaps(hbm_color, hbm_mask);
            return Err("Zero pixel count".to_string());
        }

        // Create a screen-compatible DC
        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() || hdc.0.is_null() {
            cleanup_bitmaps(hbm_color, hbm_mask);
            return Err("CreateCompatibleDC failed".to_string());
        }

        // Select the source bitmap into DC
        let old_bmp = SelectObject(hdc, src_bmp);

        // BITMAPINFO requesting 32-bit RGBA top-down DIB
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32), // negative = top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default(); 1],
        };

        let mut color_pixels: Vec<u8> = vec![0u8; pixel_count];

        // GetDIBits converts the selected bitmap to 32-bit BGRA
        let scan_lines = GetDIBits(
            hdc,
            src_bmp,
            0,
            height,
            Some(color_pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        if scan_lines == 0 {
            let _ = SelectObject(hdc, old_bmp);
            let _ = DeleteDC(hdc);
            cleanup_bitmaps(hbm_color, hbm_mask);
            return Err("GetDIBits failed for color/mask bitmap".to_string());
        }

        // If we have a separate mask bitmap and a color bitmap, apply the mask
        // to fix alpha channel (some icons have pre-multiplied alpha = 0 everywhere)
        if has_color {
            // Check if any pixel has non-zero alpha
            let has_alpha = color_pixels.chunks_exact(4).any(|px| px[3] != 0);

            if !has_alpha && !hbm_mask.is_invalid() && !hbm_mask.0.is_null() {
                // Alpha channel is all zeros — use the mask bitmap to determine transparency
                let old2 = SelectObject(hdc, hbm_mask);

                let mut mask_bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width as i32,
                        biHeight: -(height as i32),
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0 as u32,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [RGBQUAD::default(); 1],
                };

                let mut mask_pixels: Vec<u8> = vec![0u8; pixel_count];
                let mask_lines = GetDIBits(
                    hdc,
                    hbm_mask,
                    0,
                    height,
                    Some(mask_pixels.as_mut_ptr() as *mut _),
                    &mut mask_bmi,
                    DIB_RGB_COLORS,
                );

                let _ = SelectObject(hdc, old2);

                if mask_lines != 0 {
                    // Where mask is white (0xFF), the pixel is transparent
                    // Where mask is black (0x00), the pixel is opaque
                    for (color_chunk, mask_chunk) in color_pixels
                        .chunks_exact_mut(4)
                        .zip(mask_pixels.chunks_exact(4))
                    {
                        color_chunk[3] = if mask_chunk[0] == 0 { 255 } else { 0 };
                    }
                }
            }
        }

        // Cleanup GDI resources
        let _ = SelectObject(hdc, old_bmp);
        let _ = DeleteDC(hdc);
        cleanup_bitmaps(hbm_color, hbm_mask);

        // BGRA → RGBA
        for chunk in color_pixels.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        // Encode PNG
        let mut png_buf = Cursor::new(Vec::new());
        let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
        encoder
            .write_image(
                &color_pixels,
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("PNG encode failed: {e}"))?;

        let png_bytes = png_buf.into_inner();
        let data_url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png_bytes)
        );

        // Use the larger dimension as "size" for sorting
        let size = width.max(height);

        Ok(IconData {
            size,
            width,
            height,
            data_url,
        })
    }
}
