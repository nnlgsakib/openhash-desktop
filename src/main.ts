import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// DOM elements
let dbPathEl: HTMLInputElement | null;
let apiPortEl: HTMLInputElement | null;
let p2pPortEl: HTMLInputElement | null;
let startBtnEl: HTMLButtonElement | null;
let stopBtnEl: HTMLButtonElement | null;
let updateBtnEl: HTMLButtonElement | null;
let logsBtnEl: HTMLButtonElement | null;
let closeLogsBtnEl: HTMLButtonElement | null;
let statusDotEl: HTMLElement | null;
let statusTextEl: HTMLElement | null;
let logsOutputEl: HTMLElement | null;
let logsSectionEl: HTMLElement | null;
let infoMessageEl: HTMLElement | null;
let progressBarEl: HTMLProgressElement | null; // New
let progressTextEl: HTMLElement | null; // New

// Application state
let isRunning = false;
let isUpdating = false;
let logsVisible = false;
let logUpdateInterval: number | null = null;

// Initialize the application
async function initApp() {
  try {
    // Set default database path
    const defaultDbPath = await invoke("get_default_data_path");
    if (dbPathEl) {
      dbPathEl.value = defaultDbPath as string;
    }

    // Check if openhash.exe exists, if not, prompt to download
    const hasExecutable = await invoke("check_executable_exists");
    if (!hasExecutable) {
      updateInfoMessage("OpenHash executable not found. Click 'Check for Updates' to download the latest version.");
      // Listen for download progress events
      await listen<DownloadProgress>("download_progress", (event) => {
        updateProgressBar(event.payload.current, event.payload.total);
      });
      // Listen for download complete event
      await listen("download_complete", () => {
        resetProgressBar();
        updateInfoMessage("Download completed successfully. Ready to start OpenHash node.");
        updateProcessStatus(); // Re-check status after download
      });
    } else {
      updateInfoMessage("Ready to start OpenHash node.");
    }
    
    // Check process status
    await updateProcessStatus();
  } catch (error) {
    console.error("Failed to initialize app:", error);
    updateInfoMessage("Failed to initialize application.");
  }
}

interface DownloadProgress {
  current: number;
  total: number;
}

// Update progress bar
function updateProgressBar(current: number, total: number) {
  if (!progressBarEl || !progressTextEl) return;

  progressBarEl.style.display = 'block';
  progressBarEl.max = total;
  progressBarEl.value = current;

  const percentage = total > 0 ? Math.round((current / total) * 100) : 0;
  progressTextEl.textContent = `Downloading: ${percentage}%`;
}

// Reset and hide progress bar
function resetProgressBar() {
  if (!progressBarEl || !progressTextEl) return;
  progressBarEl.style.display = 'none';
  progressBarEl.value = 0;
  progressTextEl.textContent = '';
}

// Update process status from backend
async function updateProcessStatus() {
  try {
    const processRunning = await invoke("get_process_status");
    isRunning = processRunning as boolean;
    updateButtonStates();
  } catch (error) {
    console.error("Failed to get process status:", error);
  }
}

// Update info message
function updateInfoMessage(message: string) {
  if (infoMessageEl) {
    infoMessageEl.textContent = message;
  }
}

// Update status indicator
function updateStatus(status: 'stopped' | 'running' | 'updating') {
  if (!statusDotEl || !statusTextEl) return;
  
  statusDotEl.className = 'status-dot';
  
  switch (status) {
    case 'stopped':
      statusDotEl.classList.add('stopped');
      statusTextEl.textContent = 'Stopped';
      break;
    case 'running':
      statusDotEl.classList.add('running');
      statusTextEl.textContent = 'Running';
      break;
    case 'updating':
      statusDotEl.classList.add('updating');
      statusTextEl.textContent = 'Updating...';
      break;
  }
}

// Update button states
function updateButtonStates() {
  if (!startBtnEl || !stopBtnEl || !updateBtnEl) return;
  
  if (isUpdating) {
    startBtnEl.disabled = true;
    stopBtnEl.disabled = true;
    updateBtnEl.disabled = true;
    updateStatus('updating');
  } else if (isRunning) {
    startBtnEl.disabled = true;
    stopBtnEl.disabled = false;
    updateBtnEl.disabled = true;
    updateStatus('running');
  } else {
    startBtnEl.disabled = false;
    stopBtnEl.disabled = true;
    updateBtnEl.disabled = false;
    updateStatus('stopped');
  }
  
  // Disable config inputs when running
  if (dbPathEl) dbPathEl.disabled = isRunning;
  if (apiPortEl) apiPortEl.disabled = isRunning;
  if (p2pPortEl) p2pPortEl.disabled = isRunning;
}

// Start the OpenHash node
async function startNode() {
  if (!dbPathEl || !apiPortEl || !p2pPortEl) return;
  
  const config = {
    dbPath: dbPathEl.value.trim(),
    apiPort: parseInt(apiPortEl.value),
    p2pPort: parseInt(p2pPortEl.value)
  };
  
  if (!config.dbPath) {
    updateInfoMessage("Please enter a database path.");
    return;
  }
  
  if (isNaN(config.apiPort) || config.apiPort < 1 || config.apiPort > 65535) {
    updateInfoMessage("Please enter a valid API port (1-65535).");
    return;
  }
  
  if (isNaN(config.p2pPort) || config.p2pPort < 1 || config.p2pPort > 65535) {
    updateInfoMessage("Please enter a valid P2P port (1-65535).");
    return;
  }
  
  try {
    updateInfoMessage("Starting OpenHash node...");
    const result = await invoke("start_node", { config });
    
    if (result) {
      isRunning = true;
      updateButtonStates();
      updateInfoMessage("OpenHash node started successfully.");
      
      // Start monitoring logs if logs are visible
      if (logsVisible) {
        startLogMonitoring();
      }
    } else {
      updateInfoMessage("Failed to start OpenHash node.");
    }
  } catch (error) {
    console.error("Failed to start node:", error);
    updateInfoMessage(`Failed to start node: ${error}`);
  }
}

// Stop the OpenHash node
async function stopNode() {
  try {
    updateInfoMessage("Stopping OpenHash node...");
    const result = await invoke("stop_node");
    
    if (result) {
      isRunning = false;
      updateButtonStates();
      updateInfoMessage("OpenHash node stopped successfully.");
      
      // Stop log monitoring
      stopLogMonitoring();
    } else {
      updateInfoMessage("Failed to stop OpenHash node.");
    }
  } catch (error) {
    console.error("Failed to stop node:", error);
    updateInfoMessage(`Failed to stop node: ${error}`);
  }
}

// Check for updates and download if available
async function checkForUpdates() {
  try {
    isUpdating = true;
    updateButtonStates();
    updateInfoMessage("Checking for updates and downloading openhash.exe...");
    
    await invoke("check_and_download_update");
    
    // Success message will be handled by the download_complete event listener
  } catch (error) {
    console.error("Failed to check for updates:", error);
    updateInfoMessage(`Update failed: ${error}`);
    resetProgressBar(); // Hide progress bar on error
  } finally {
    isUpdating = false;
    updateButtonStates();
  }
}

// Toggle logs visibility
function toggleLogs() {
  if (!logsSectionEl) return;
  
  logsVisible = !logsVisible;
  logsSectionEl.style.display = logsVisible ? 'block' : 'none';
  
  if (logsVisible) {
    updateLogs();
    if (isRunning) {
      startLogMonitoring();
    }
  } else {
    stopLogMonitoring();
  }
}

// Start log monitoring (periodic updates)
function startLogMonitoring() {
  if (logUpdateInterval) return; // Already monitoring
  
  logUpdateInterval = window.setInterval(updateLogs, 1000); // Update every second
}

// Stop log monitoring
function stopLogMonitoring() {
  if (logUpdateInterval) {
    clearInterval(logUpdateInterval);
    logUpdateInterval = null;
  }
}

// Update logs display
async function updateLogs() {
  if (!logsOutputEl || !logsVisible) return;
  
  try {
    const logs = await invoke("get_logs");
    const logsText = logs as string;
    
    // Only update if content has changed
    if (logsOutputEl.textContent !== logsText) {
      logsOutputEl.textContent = logsText;
      
      // Auto-scroll to bottom
      const logsContainer = logsOutputEl.parentElement;
      if (logsContainer) {
        logsContainer.scrollTop = logsContainer.scrollHeight;
      }
    }
  } catch (error) {
    console.error("Failed to get logs:", error);
    logsOutputEl.textContent = "Failed to retrieve logs.";
  }
}

// Clear logs
async function clearLogs() {
  try {
    await invoke("clear_logs");
    if (logsOutputEl) {
      logsOutputEl.textContent = "";
    }
  } catch (error) {
    console.error("Failed to clear logs:", error);
  }
}

// Event listeners
window.addEventListener("DOMContentLoaded", () => {
  // Get DOM elements
  dbPathEl = document.querySelector("#db-path");
  apiPortEl = document.querySelector("#api-port");
  p2pPortEl = document.querySelector("#p2p-port");
  startBtnEl = document.querySelector("#start-btn");
  stopBtnEl = document.querySelector("#stop-btn");
  updateBtnEl = document.querySelector("#update-btn");
  logsBtnEl = document.querySelector("#logs-btn");
  closeLogsBtnEl = document.querySelector("#close-logs-btn");
  statusDotEl = document.querySelector("#status-dot");
  statusTextEl = document.querySelector("#status-text");
  logsOutputEl = document.querySelector("#logs-output");
  logsSectionEl = document.querySelector("#logs-section");
  infoMessageEl = document.querySelector("#info-message");
  progressBarEl = document.querySelector("#download-progress-bar"); // New
  progressTextEl = document.querySelector("#download-progress-text"); // New
  
  // Add event listeners
  startBtnEl?.addEventListener("click", startNode);
  stopBtnEl?.addEventListener("click", stopNode);
  updateBtnEl?.addEventListener("click", checkForUpdates);
  logsBtnEl?.addEventListener("click", toggleLogs);
  closeLogsBtnEl?.addEventListener("click", toggleLogs);
  
  // Add double-click to clear logs
  logsOutputEl?.addEventListener("dblclick", clearLogs);
  
  // Initialize the application
  initApp();
  updateButtonStates();
  
  // Periodic status check
  setInterval(updateProcessStatus, 5000); // Check every 5 seconds
});

