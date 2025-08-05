use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<u64>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn list_directory(dir_path: String) -> Result<Vec<FileInfo>, String> {
    let path = Path::new(&dir_path);
    if !path.exists() {
        return Err(format!("Directory does not exist: {}", dir_path));
    }
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", dir_path));
    }
    let mut files = Vec::new();
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().and_then(|m| if m.is_file() { Some(m.len()) } else { None });
                    let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    let modified = metadata.as_ref().and_then(|m| {
                        m.modified().ok().and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
                        })
                    });
                    
                    let file_info = FileInfo {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: entry.path().to_string_lossy().to_string(),
                        is_dir,
                        size,
                        modified,
                    };
                    
                    files.push(file_info);
                }
            }
            Ok(files)
        }
        Err(e) => Err(format!("Failed to read directory: {}", e))
    }
}

#[tauri::command]
async fn move_file(source_path: String, destination_path: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let destination = Path::new(&destination_path);
    if !source.exists() { return Err(format!("Source file does not exist: {}", source_path)); }
    if !source.is_file() { return Err(format!("Source path is not a file: {}", source_path)); }
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create destination directory: {}", e));
            }
        }
    }
    if destination.exists() { return Err(format!("Destination file already exists: {}", destination_path)); }
    match fs::rename(source, destination) {
        Ok(_) => Ok(format!("File moved successfully from {} to {}", source_path, destination_path)),
        Err(e) => Err(format!("Failed to move file: {}", e))
    }
}

#[tauri::command]
async fn copy_file(source_path: String, destination_path: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let destination = Path::new(&destination_path);
    if !source.exists() { return Err(format!("Source file does not exist: {}", source_path)); }
    if !source.is_file() { return Err(format!("Source path is not a file: {}", source_path)); }
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create destination directory: {}", e));
            }
        }
    }
    match fs::copy(source, destination) {
        Ok(_) => Ok(format!("File copied successfully from {} to {}", source_path, destination_path)),
        Err(e) => Err(format!("Failed to copy file: {}", e))
    }
}

#[tauri::command]
async fn file_exists(file_path: String) -> bool {
    Path::new(&file_path).exists()
}

#[tauri::command]
async fn get_file_info(file_path: String) -> Result<serde_json::Value, String> {
    let path = Path::new(&file_path);
    if !path.exists() { return Err(format!("File does not exist: {}", file_path)); }
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(format!("Failed to get file metadata: {}", e))
    };
    let file_info = serde_json::json!({
        "name": path.file_name().unwrap_or_default().to_string_lossy(),
        "path": path.to_string_lossy(),
        "size": metadata.len(),
        "is_file": metadata.is_file(),
        "is_dir": metadata.is_dir(),
        "modified": metadata.modified().unwrap_or_else(|_| std::time::SystemTime::now()).duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
    });
    Ok(file_info)
}

#[tauri::command]
async fn read_video_file(file_path: String) -> Result<String, String> {
    let path = Path::new(&file_path);
    if !path.exists() { return Err(format!("File does not exist: {}", file_path)); }
    if !path.is_file() { return Err(format!("Path is not a file: {}", file_path)); }
    
    match fs::read(path) {
        Ok(data) => {
            let base64_data = general_purpose::STANDARD.encode(&data);
            Ok(base64_data)
        },
        Err(e) => Err(format!("Failed to read file: {}", e))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            list_directory,
            move_file,
            copy_file,
            file_exists,
            get_file_info,
            read_video_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
