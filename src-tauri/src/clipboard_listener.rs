use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::thread;
use std::time::Duration;

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
struct ClipboardUpdate {
    #[serde(rename = "type")]
    item_type: String,
    content: String,
}

fn image_to_data_url(img: ImageData<'static>) -> Option<(u64, String)> {
    let mut hasher = DefaultHasher::new();
    img.width.hash(&mut hasher);
    img.height.hash(&mut hasher);
    img.bytes.len().hash(&mut hasher);
    if !img.bytes.is_empty() {
        img.bytes[0].hash(&mut hasher);
        img.bytes[img.bytes.len() / 2].hash(&mut hasher);
        img.bytes[img.bytes.len() - 1].hash(&mut hasher);
    }
    let hash = hasher.finish();

    let rgba = image::RgbaImage::from_raw(
        img.width as u32,
        img.height as u32,
        img.into_owned_bytes().into_owned(),
    )?;

    let mut png_bytes = Vec::new();
    let dyn_img = image::DynamicImage::ImageRgba8(rgba);
    dyn_img
        .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .ok()?;
    let b64 = general_purpose::STANDARD.encode(png_bytes);
    let data_url = format!("data:image/png;base64,{b64}");
    Some((hash, data_url))
}

pub fn start(app: AppHandle) {
    thread::spawn(move || {
        let clipboard = Clipboard::new();
        if clipboard.is_err() {
            eprintln!("Failed to init clipboard: {:?}", clipboard.err());
            return;
        }
        let mut clipboard = clipboard.unwrap();

        let mut last_text = String::new();
        let mut last_image_hash: u64 = 0;

        // Initialize with current content to avoid re-triggering on startup?
        // Or trigger it to populate list?
        // Let's trigger it.

        if let Ok(content) = clipboard.get_text() {
            last_text = content.clone();
            let _ = app.emit(
                "clipboard-update",
                ClipboardUpdate {
                    item_type: "text".to_string(),
                    content,
                },
            );
        } else if let Ok(img) = clipboard.get_image() {
            if let Some((hash, data_url)) = image_to_data_url(img) {
                last_image_hash = hash;
                let _ = app.emit(
                    "clipboard-update",
                    ClipboardUpdate {
                        item_type: "image".to_string(),
                        content: data_url,
                    },
                );
            }
        }

        loop {
            if let Ok(content) = clipboard.get_text() {
                if content != last_text && !content.is_empty() {
                    last_text = content.clone();
                    let _ = app.emit(
                        "clipboard-update",
                        ClipboardUpdate {
                            item_type: "text".to_string(),
                            content,
                        },
                    );
                }
            } else if let Ok(img) = clipboard.get_image() {
                if let Some((hash, data_url)) = image_to_data_url(img) {
                    if hash != last_image_hash {
                        last_image_hash = hash;
                        last_text.clear();
                        let _ = app.emit(
                            "clipboard-update",
                            ClipboardUpdate {
                                item_type: "image".to_string(),
                                content: data_url,
                            },
                        );
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}
