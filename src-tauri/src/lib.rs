use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<f64>,
    bitrate: Option<String>,
    codec: Option<String>,
    fps: Option<f32>,
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
async fn directory_exists(dir_path: String) -> bool {
    let path = Path::new(&dir_path);
    path.exists() && path.is_dir()
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
    
    // Check file size first to avoid loading huge files
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Err(format!("Failed to get file metadata: {}", e))
    };
    
    // Limit to files smaller than 50MB to prevent memory issues
    if metadata.len() > 50 * 1024 * 1024 {
        return Err("File too large for thumbnail generation (max 50MB)".to_string());
    }
    
    match fs::read(path) {
        Ok(data) => {
            let base64_data = general_purpose::STANDARD.encode(&data);
            Ok(base64_data)
        },
        Err(e) => Err(format!("Failed to read file: {}", e))
    }
}

#[tauri::command]
async fn generate_video_thumbnails(file_path: String) -> Result<Vec<String>, String> {
    let path = Path::new(&file_path);
    if !path.exists() { return Err(format!("File does not exist: {}", file_path)); }
    if !path.is_file() { return Err(format!("Path is not a file: {}", file_path)); }
    
    // Check if ffmpeg is available
    let ffmpeg_check = Command::new("ffmpeg").arg("-version").output();
    if ffmpeg_check.is_err() {
        // FFmpeg not available - return empty thumbnails with a message
        println!("FFmpeg not found - thumbnails will not be generated");
        return Ok(vec!["".to_string(); 5]);
    }
    
    // Check if ffprobe is available
    let ffprobe_check = Command::new("ffprobe").arg("-version").output();
    if ffprobe_check.is_err() {
        // FFprobe not available - return empty thumbnails with a message
        println!("FFprobe not found - thumbnails will not be generated");
        return Ok(vec!["".to_string(); 5]);
    }
    
    // Get video duration
    let duration_output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "csv=p=0",
            &file_path
        ])
        .output();
    
    let duration_str = match duration_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !output.status.success() {
                println!("FFprobe failed: {}", stderr);
                return Ok(vec!["".to_string(); 5]);
            }
            stdout.trim().to_string()
        },
        Err(e) => {
            println!("FFprobe error: {}", e);
            return Ok(vec!["".to_string(); 5]);
        },
    };
    
    let duration: f64 = match duration_str.parse() {
        Ok(d) => {
            if d <= 0.0 {
                println!("Invalid duration: {}", d);
                return Ok(vec!["".to_string(); 5]);
            }
            d
        },
        Err(_) => {
            println!("Could not parse duration: '{}'", duration_str);
            return Ok(vec!["".to_string(); 5]);
        },
    };
    
    // Generate thumbnails at different timestamps
    let timestamps = vec![duration * 0.1, duration * 0.2, duration * 0.3, duration * 0.4, duration * 0.5];
    let mut thumbnails = Vec::new();
    
    for (_i, timestamp) in timestamps.iter().enumerate() {
        let frame_output = Command::new("ffmpeg")
            .args(&[
                "-ss", &timestamp.to_string(),
                "-i", &file_path,
                "-vframes", "1",
                "-f", "mjpeg",
                "-vf", "scale=320:240",
                "-q:v", "2",
                "-y", // Overwrite output
                "-" // Output to stdout
            ])
            .output();
        
        match frame_output {
            Ok(output) => {
                if output.status.success() && !output.stdout.is_empty() {
                    let base64_data = general_purpose::STANDARD.encode(&output.stdout);
                    thumbnails.push(base64_data);
                } else {
                    // If frame extraction fails, try a simpler approach
                    let simple_output = Command::new("ffmpeg")
                        .args(&[
                            "-ss", &timestamp.to_string(),
                            "-i", &file_path,
                            "-vframes", "1",
                            "-f", "image2",
                            "-vf", "scale=320:240",
                            "-y",
                            "-"
                        ])
                        .output();
                    
                    match simple_output {
                        Ok(simple_result) => {
                            if simple_result.status.success() && !simple_result.stdout.is_empty() {
                                let base64_data = general_purpose::STANDARD.encode(&simple_result.stdout);
                                thumbnails.push(base64_data);
                            } else {
                                // If still fails, add empty string
                                thumbnails.push("".to_string());
                            }
                        },
                        Err(_) => {
                            thumbnails.push("".to_string());
                        }
                    }
                }
            },
            Err(_) => {
                // If frame extraction fails, add a placeholder
                thumbnails.push("".to_string());
            }
        }
    }
    
    Ok(thumbnails)
}

#[tauri::command]
async fn open_file(file_path: String) -> Result<(), String> {
    use std::process::Command;
    
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    
    Ok(())
}

#[tauri::command]
async fn get_video_metadata(file_path: String) -> Result<VideoMetadata, String> {
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_streams",
            "-show_format",
            &file_path
        ])
        .output();
    
    match output {
        Ok(output) => {
            if output.status.success() {
                let json_str = String::from_utf8_lossy(&output.stdout);
                
                // Parse the JSON response
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let mut metadata = VideoMetadata {
                        width: None,
                        height: None,
                        duration: None,
                        bitrate: None,
                        codec: None,
                        fps: None,
                    };
                    
                    // Extract video stream information
                    if let Some(streams) = json["streams"].as_array() {
                        for stream in streams {
                            if stream["codec_type"] == "video" {
                                // Get resolution
                                if let Some(width) = stream["width"].as_u64() {
                                    metadata.width = Some(width as u32);
                                }
                                if let Some(height) = stream["height"].as_u64() {
                                    metadata.height = Some(height as u32);
                                }
                                
                                // Get codec
                                if let Some(codec) = stream["codec_name"].as_str() {
                                    metadata.codec = Some(codec.to_string());
                                }
                                
                                // Get FPS
                                if let Some(fps_str) = stream["r_frame_rate"].as_str() {
                                    let parts: Vec<&str> = fps_str.split('/').collect();
                                    if parts.len() == 2 {
                                        if let (Ok(num), Ok(den)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                                            if den != 0.0 {
                                                metadata.fps = Some(num / den);
                                            }
                                        }
                                    }
                                }
                                break; // We only need the first video stream
                            }
                        }
                    }
                    
                    // Extract format information
                    if let Some(format) = json["format"].as_object() {
                        // Get duration
                        if let Some(duration_str) = format["duration"].as_str() {
                            if let Ok(duration) = duration_str.parse::<f64>() {
                                metadata.duration = Some(duration);
                            }
                        }
                        
                        // Get bitrate
                        if let Some(bitrate) = format["bit_rate"].as_str() {
                            metadata.bitrate = Some(bitrate.to_string());
                        }
                    }
                    
                    Ok(metadata)
                } else {
                    Err("Failed to parse video metadata JSON".to_string())
                }
            } else {
                Err("FFprobe command failed".to_string())
            }
        },
        Err(e) => Err(format!("Failed to execute ffprobe: {}", e))
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
            directory_exists,
            get_file_info,
            read_video_file,
            generate_video_thumbnails,
            get_video_metadata,
            open_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
