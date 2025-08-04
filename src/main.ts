import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

// DOM elements
let dbPathEl: HTMLInputElement | null;
let dbPathTextEl: HTMLElement | null;
let dbPathContainerEl: HTMLElement | null;
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
let progressBarEl: HTMLProgressElement | null;
let progressTextEl: HTMLElement | null;

// Application state
let isRunning = false;
let isUpdating = false;
let logsVisible = false;
let logUpdateInterval: number | null = null;

// Initialize the application
async function initApp() {
  try {
    // Set default database path
    await updateDbPath();

    // Check if openhash.exe exists in the specified path
    const hasExecutable = await invoke("check_executable_exists", {
      dbPath: dbPathEl?.value,
    });

    if (!hasExecutable) {
      updateInfoMessage("OpenHash executable not found. Click 'Check for Updates' to download the latest version.");
    } else {
      updateInfoMessage("Ready to start OpenHash node.");
    }
    
    // Listen for download events
    await listen<DownloadProgress>("download_progress", (event) => {
      updateProgressBar(event.payload.current, event.payload.total);
    });
    await listen("download_complete", () => {
      resetProgressBar();
      updateInfoMessage("Download completed successfully. Ready to start OpenHash node.");
      updateProcessStatus(); // Re-check status after download
    });

    // Check process status
    await updateProcessStatus();
  } catch (error) {
    console.error("Failed to initialize app:", error);
    updateInfoMessage("Failed to initialize application.");
  }
}

// Fetches and updates the database path from the backend
async function updateDbPath() {
  try {
    const currentPath = await invoke("get_current_data_path");
    if (dbPathEl && dbPathTextEl) {
      dbPathEl.value = currentPath as string;
      dbPathTextEl.textContent = currentPath as string;
    }
  } catch (error) {
    console.error("Failed to get data path:", error);
    updateInfoMessage("Failed to get data path.");
  }
}

// Opens a dialog to select a new data path
async function selectCustomPath() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select a Data Directory",
    });

    if (typeof selected === 'string' && selected.trim() !== '') {
      await invoke("set_custom_data_path", { path: selected });
      await updateDbPath(); // Refresh the displayed path
      updateInfoMessage(`Data path set to: ${selected}`);
    }
  } catch (error) {
    console.error("Failed to select custom path:", error);
    updateInfoMessage("Failed to set custom data path.");
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
  if (!startBtnEl || !stopBtnEl || !updateBtnEl || !dbPathContainerEl) return;
  
  const disableInputs = isRunning || isUpdating;

  if (isUpdating) {
    updateStatus('updating');
  } else if (isRunning) {
    updateStatus('running');
  } else {
    updateStatus('stopped');
  }

  startBtnEl.disabled = disableInputs;
  stopBtnEl.disabled = !isRunning || isUpdating;
  updateBtnEl.disabled = disableInputs;
  
  // Disable config inputs and path selection when running or updating
  if (apiPortEl) apiPortEl.disabled = disableInputs;
  if (p2pPortEl) p2pPortEl.disabled = disableInputs;
  
  if (disableInputs) {
    dbPathContainerEl.classList.add('disabled');
    dbPathContainerEl.removeEventListener('click', selectCustomPath);
  } else {
    dbPathContainerEl.classList.remove('disabled');
    dbPathContainerEl.addEventListener('click', selectCustomPath);
  }
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
    updateInfoMessage("Database path is not set.");
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
  if (!dbPathEl) return;

  try {
    isUpdating = true;
    updateButtonStates();
    updateInfoMessage("Checking for updates and downloading openhash.exe...");
    
    await invoke("check_and_download_update", {
      dbPath: dbPathEl.value.trim(),
    });
    
  } catch (error) {
    console.error("Failed to check for updates:", error);
    updateInfoMessage(`Update failed: ${error}`);
    resetProgressBar();
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
  if (logUpdateInterval) return;
  logUpdateInterval = window.setInterval(updateLogs, 1000);
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
    
    if (logsOutputEl.textContent !== logsText) {
      logsOutputEl.textContent = logsText;
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
  dbPathTextEl = document.querySelector("#db-path-text");
  dbPathContainerEl = document.querySelector("#db-path-container");
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
  progressBarEl = document.querySelector("#download-progress-bar");
  progressTextEl = document.querySelector("#download-progress-text");
  
  // Add event listeners
  startBtnEl?.addEventListener("click", startNode);
  stopBtnEl?.addEventListener("click", stopNode);
  updateBtnEl?.addEventListener("click", checkForUpdates);
  logsBtnEl?.addEventListener("click", toggleLogs);
  closeLogsBtnEl?.addEventListener("click", toggleLogs);
  dbPathContainerEl?.addEventListener("click", selectCustomPath);
  logsOutputEl?.addEventListener("dblclick", clearLogs);
  
  // Initialize the application
  initApp();
  updateButtonStates();
  
  // Periodic status check
  setInterval(updateProcessStatus, 5000);
});

