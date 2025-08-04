use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::fs;
use std::io::{BufRead, BufReader};
use std::thread;
use serde::{Deserialize, Serialize};
use tauri::State;

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
fn get_executable_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove the executable name
    path.push("openhash.exe");
    path
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
    get_executable_path().exists()
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
    let executable_path = get_executable_path();
    
    if !executable_path.exists() {
        return Err("OpenHash executable not found. Please download it first.".to_string());
    }
    
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
       .arg(&config.db_path)
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
            add_log_entry(&state.logs, &format!("Starting OpenHash node with config: {:?}", config));
            
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
async fn check_and_download_update(state: State<'_, AppState>) -> Result<bool, String> {
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
    
    add_log_entry(&state.logs, "Downloading openhash.exe...");
    
    // Download the executable
    let download_response = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to download executable: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
    if !download_response.status().is_success() {
        let error_msg = "Failed to download executable".to_string();
        add_log_entry(&state.logs, &error_msg);
        return Err(error_msg);
    }
    
    let executable_bytes = download_response
        .bytes()
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to read executable bytes: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
    // Save the executable
    let executable_path = get_executable_path();
    fs::write(&executable_path, executable_bytes)
        .map_err(|e| {
            let error_msg = format!("Failed to save executable: {}", e);
            add_log_entry(&state.logs, &error_msg);
            error_msg
        })?;
    
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
    Ok(true)
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
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            check_executable_exists,
            get_process_status,
            start_node,
            stop_node,
            check_and_download_update,
            get_logs,
            clear_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

