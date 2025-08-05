use std::path::Path;
use std::fs;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn move_file(source_path: String, destination_path: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let destination = Path::new(&destination_path);
    
    // Check if source file exists
    if !source.exists() {
        return Err(format!("Source file does not exist: {}", source_path));
    }
    
    // Check if source is actually a file
    if !source.is_file() {
        return Err(format!("Source path is not a file: {}", source_path));
    }
    
    // Create destination directory if it doesn't exist
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create destination directory: {}", e));
            }
        }
    }
    
    // Check if destination already exists
    if destination.exists() {
        return Err(format!("Destination file already exists: {}", destination_path));
    }
    
    // Move the file
    match fs::rename(source, destination) {
        Ok(_) => Ok(format!("File moved successfully from {} to {}", source_path, destination_path)),
        Err(e) => Err(format!("Failed to move file: {}", e))
    }
}

#[tauri::command]
async fn copy_file(source_path: String, destination_path: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let destination = Path::new(&destination_path);
    
    // Check if source file exists
    if !source.exists() {
        return Err(format!("Source file does not exist: {}", source_path));
    }
    
    // Check if source is actually a file
    if !source.is_file() {
        return Err(format!("Source path is not a file: {}", source_path));
    }
    
    // Create destination directory if it doesn't exist
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Err(format!("Failed to create destination directory: {}", e));
            }
        }
    }
    
    // Copy the file
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
    
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path));
    }
    
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            move_file,
            copy_file,
            file_exists,
            get_file_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
