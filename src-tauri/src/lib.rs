use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::fs;
use std::io::{BufRead, BufReader};
use std::thread;
use serde::{Deserialize, Serialize};
use tauri::{State, Emitter}; // Added Emitter back
use futures_util::StreamExt; // For stream processing
use tokio::io::AsyncWriteExt; // For async file writing
use tokio::fs::OpenOptions; // Added for OpenOptions
use dirs;
use reqwest::header; // Added for Range header

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    #[serde(rename = "dbPath")]
    db_path: String,
    #[serde(rename = "apiPort")]
    api_port: u16,
    #[serde(rename = "p2pPort")]
    p2p_port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

// Application state to manage the running process
pub struct AppState {
    pub process: Arc<Mutex<Option<Child>>>,
    pub logs: Arc<Mutex<String>>,
    pub is_running: Arc<Mutex<bool>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            logs: Arc::new(Mutex::new(String::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }
}

// Get the path to the openhash executable
fn get_executable_path(download_dir: Option<PathBuf>) -> PathBuf {
    let mut path = if let Some(dir) = download_dir {
        dir
    } else {
        dirs::data_dir().unwrap_or_else(|| {
            let mut p = std::env::current_exe().unwrap();
            p.pop(); // Remove the executable name
            p
        })
    };
    path.push("openhash.exe");
    path
}

// Get the default data directory for the application
#[tauri::command]
fn get_default_data_path() -> String {
    if let Some(mut path) = dirs::data_dir() {
        path.push("OpenHash"); // Subdirectory for your app
        path.to_string_lossy().into_owned()
    } else {
        "data/data1/node1".to_string() // Fallback if data_dir is not found
    }
}

// Add a log entry with timestamp
fn add_log_entry(logs: &Arc<Mutex<String>>, message: &str) {
    let mut logs_guard = logs.lock().unwrap();
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    logs_guard.push_str(&format!("[{}] {}\n", timestamp, message));
    
    // Keep only the last 1000 lines to prevent memory issues
    let lines: Vec<&str> = logs_guard.lines().collect();
    if lines.len() > 1000 {
        let keep_lines = &lines[lines.len() - 1000..];
        *logs_guard = keep_lines.join("\n") + "\n";
    }
}

// Check if the openhash executable exists
#[tauri::command]
fn check_executable_exists() -> bool {
    get_executable_path(None).exists()
}

// Get the current process status
#[tauri::command]
async fn get_process_status(state: State<'_, AppState>) -> Result<bool, String> {
    let is_running = state.is_running.lock().unwrap();
    Ok(*is_running)
}

// Start the OpenHash node
#[tauri::command]
async fn start_node(config: NodeConfig, state: State<'_, AppState>) -> Result<bool, String> {
    let executable_path = get_executable_path(None);
    
    if !executable_path.exists() {
        return Err("OpenHash executable not found. Please download it first.".to_string());
    }
    
    // If db_path is empty, use the default data directory
    let final_db_path = if config.db_path.is_empty() {
        let mut default_path = dirs::data_dir().unwrap_or_else(|| {
            let mut p = std::env::current_exe().unwrap();
            p.pop();
            p
        });
        default_path.push("OpenHash");
        default_path.push("data1"); // Example subdirectory
        default_path.push("node1"); // Example subdirectory
        fs::create_dir_all(&default_path).map_err(|e| format!("Failed to create default DB directory: {}", e))?;
        default_path.to_string_lossy().into_owned()
    } else {
        config.db_path.clone() // Clone to avoid partial move
    };
    
    // Check if a process is already running
    {
        let is_running = state.is_running.lock().unwrap();
        if *is_running {
            return Err("Node is already running".to_string());
        }
    }
    
    // Build the command
    let mut cmd = Command::new(&executable_path);
    cmd.arg("daemon")
       .arg("--api-port")
       .arg(config.api_port.to_string())
       .arg("--db")
       .arg(&final_db_path)
       .arg("--p2p-port")
       .arg(config.p2p_port.to_string())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());
    
    // Start the process
    match cmd.spawn() {
        Ok(mut child) => {
            // Set running status
            {
                let mut is_running = state.is_running.lock().unwrap();
                *is_running = true;
            }
            
            // Clear previous logs and add startup message
            {
                let mut logs_guard = state.logs.lock().unwrap();
                logs_guard.clear();
            }
            add_log_entry(&state.logs, &format!("Starting OpenHash node with config: {:?}, DB Path: {}", &config, final_db_path));
            
            // Capture stdout
            if let Some(stdout) = child.stdout.take() {
                let logs_clone = Arc::clone(&state.logs);
                let is_running_clone = Arc::clone(&state.is_running);
                thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        match line {
                            Ok(line) => {
                                add_log_entry(&logs_clone, &format!("STDOUT: {}", line));
                            }
                            Err(_) => break,
                        }
                        
                        // Check if process is still supposed to be running
                        let is_running = is_running_clone.lock().unwrap();
                        if !*is_running {
                            break;
                        }
                    }
                });
            }
            
            // Capture stderr
            if let Some(stderr) = child.stderr.take() {
                let logs_clone = Arc::clone(&state.logs);
                let is_running_clone = Arc::clone(&state.is_running);
                thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        match line {
                            Ok(line) => {
                                add_log_entry(&logs_clone, &format!("STDERR: {}", line));
                            }
                            Err(_) => break,
                        }
                        
                        // Check if process is still supposed to be running
                        let is_running = is_running_clone.lock().unwrap();
                        if !*is_running {
                            break;
                        }
                    }
                });
            }
            
            // Store the process
            let mut process_guard = state.process.lock().unwrap();
            *process_guard = Some(child);
            
            add_log_entry(&state.logs, "OpenHash node started successfully");
            Ok(true)
        }
        Err(e) => {
            add_log_entry(&state.logs, &format!("Failed to start process: {}", e));
            Err(format!("Failed to start process: {}", e))
        }
    }
}

// Stop the OpenHash node
#[tauri::command]
async fn stop_node(state: State<'_, AppState>) -> Result<bool, String> {
    // Set running status to false first
    {
        let mut is_running = state.is_running.lock().unwrap();
        *is_running = false;
    }
    
    let mut process_guard = state.process.lock().unwrap();
    
    if let Some(mut child) = process_guard.take() {
        match child.kill() {
            Ok(_) => {
                // Wait for the process to terminate
                let _ = child.wait();
                
                add_log_entry(&state.logs, "OpenHash node stopped");
                Ok(true)
            }
            Err(e) => {
                add_log_entry(&state.logs, &format!("Failed to stop process: {}", e));
                Err(format!("Failed to stop process: {}", e))
            }
        }
    } else {
        add_log_entry(&state.logs, "No running process found");
        Err("No running process found".to_string())
    }
}

// Check for updates and download if available
#[tauri::command]
async fn check_and_download_update(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    const GITHUB_API_URL: &str = "https://api.github.com/repos/nnlgsakib/open-hash-db/releases/latest";
    
    add_log_entry(&state.logs, "Checking for updates...");
    
    // Fetch the latest release information
    let client = reqwest::Client::new();
    let response = client
        .get(GITHUB_API_URL)
        .header("User-Agent", "OpenHash-Wrapper")
        .send()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to fetch release info: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
    if !response.status().is_success() {
        let error_msg = "Failed to fetch release information from GitHub".to_string();
        add_log_entry(&state.logs, &error_msg);
        return Err(error_msg);
    }
    
    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to parse release info: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
    add_log_entry(&state.logs, &format!("Found release: {}", release.tag_name));
    
    // Find the openhash.exe asset
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == "openhash.exe")
        .ok_or_else(|| {
            let error_msg = "openhash.exe not found in release assets".to_string();
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
    // Determine the executable path (default to app data directory)
    let executable_path = get_executable_path(None);
    let mut downloaded_bytes: u64 = 0;

    // Get total size from HEAD request first
    let head_response = client
        .head(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to get file size: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    let total_size = head_response.content_length().unwrap_or(0);

    // Check if a partial file exists and get its size for resuming
    if executable_path.exists() {
        match fs::metadata(&executable_path) {
            Ok(metadata) => {
                downloaded_bytes = metadata.len();
                if downloaded_bytes == total_size {
                    add_log_entry(&state.logs, "openhash.exe is already up to date.");
                    app_handle.emit("download_complete", ()).map_err(|e| {
                        let error_msg = format!("Failed to emit download_complete event: {}", e);
                        add_log_entry(&state.logs, &error_msg);
                        error_msg
                    })?;
                    return Ok(true);
                } else if downloaded_bytes < total_size {
                    add_log_entry(&state.logs, &format!("Resuming download from {} bytes.", downloaded_bytes));
                } else { // downloaded_bytes > total_size, likely a corrupted or newer file
                    add_log_entry(&state.logs, "Existing file is larger than expected, restarting download.");
                    fs::remove_file(&executable_path).map_err(|e| format!("Failed to remove corrupted file: {}", e))?;
                    downloaded_bytes = 0;
                }
            },
            Err(e) => {
                let error_msg = format!("Failed to get metadata for existing file: {}", e);
                add_log_entry(&state.logs, &error_msg);
                return Err(error_msg);
            }
        }
    }

    add_log_entry(&state.logs, &format!("Downloading openhash.exe to {:?}...", executable_path));
    
    // Download the executable with progress and resumability
    let mut request_builder = client.get(&asset.browser_download_url);
    if downloaded_bytes > 0 {
        request_builder = request_builder.header(reqwest::header::RANGE, format!("bytes={}-", downloaded_bytes));
    }

    let download_response = request_builder
        .send()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to download executable: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
    if !download_response.status().is_success() && download_response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        let error_msg = format!("Failed to download executable: Status {}", download_response.status());
        add_log_entry(&state.logs, &error_msg);
        return Err(error_msg);
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true) // Append to existing file for resumability
        .open(&executable_path)
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to open file for writing: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;

    let mut stream = download_response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            let error_msg = format!("Error while downloading chunk: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
        file.write_all(&chunk)
            .await
            .map_err(|e| {
                let error_msg = format!("Error while writing to file: {}", e);
                add_log_entry(&state.logs, &error_msg);
                error_msg
            })?;
        downloaded_bytes += chunk.len() as u64;

        // Emit progress event
        app_handle.emit("download_progress", DownloadProgress {
            current: downloaded_bytes,
            total: total_size,
        }).map_err(|e| {
            let error_msg = format!("Failed to emit download_progress event: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    }
    
    // Make it executable on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&executable_path)
            .map_err(|e| {
                let error_msg = format!("Failed to get file metadata: {}", e);
                add_log_entry(&state.logs, &error_msg);
                error_msg
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&executable_path, perms)
            .map_err(|e| {
                let error_msg = format!("Failed to set executable permissions: {}", e);
                add_log_entry(&state.logs, &error_msg);
                error_msg
            })?;
    }
    
    add_log_entry(&state.logs, "Download completed successfully");
    app_handle.emit("download_complete", ()).map_err(|e| {
        let error_msg = format!("Failed to emit download_complete event: {}", e);
        add_log_entry(&state.logs, &error_msg);
        error_msg
    })?;
    Ok(true)
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    current: u64,
    total: u64,
}

// Get logs from the running process
#[tauri::command]
async fn get_logs(state: State<'_, AppState>) -> Result<String, String> {
    let logs_guard = state.logs.lock().unwrap();
    Ok(logs_guard.clone())
}

// Clear logs
#[tauri::command]
async fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut logs_guard = state.logs.lock().unwrap();
    logs_guard.clear();
    Ok(())
}

// Legacy greet command (keeping for compatibility)
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            check_executable_exists,
            get_process_status,
            start_node,
            stop_node,
            check_and_download_update, // Re-typed
            get_logs,
            clear_logs,
            get_default_data_path
        ])
        .setup(|_app| {
            #[cfg(debug_assertions)] // only enable for debug builds
            {
                let window = app.get_window("main").unwrap();
                window.open_devtools();
                window.close_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

