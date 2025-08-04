# OpenHash Wrapper

A user-friendly desktop application wrapper for openhash.exe, built with Tauri.

## Features

- **Easy Configuration**: Simple GUI for setting database path, API port, and P2P port
- **Start/Stop Controls**: One-click start and stop functionality with proper state management
- **Auto-Updates**: Automatically downloads the latest openhash.exe from GitHub releases
- **Live Logs**: Real-time log viewing with timestamps and auto-scrolling
- **Process Monitoring**: Visual status indicators and process state tracking
- **User-Friendly**: Designed for non-technical users with clear interface and error messages

## Prerequisites

### System Requirements
- Rust (latest stable version)
- Node.js (v16 or later)
- npm or yarn

### Linux Dependencies
```bash
sudo apt update
sudo apt install -y \
    build-essential \
    libwebkit2gtk-4.0-dev \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libssl-dev \
    libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev
```

### Windows Dependencies
- Microsoft Visual Studio C++ Build Tools
- WebView2 (usually pre-installed on Windows 10/11)

### macOS Dependencies
- Xcode Command Line Tools

## Installation & Build

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd openhash-desktop
   ```

2. **Install dependencies**
   ```bash
   npm install
   ```

3. **Development mode**
   ```bash
   npm run tauri dev
   ```

4. **Build for production**
   ```bash
   npm run tauri build
   ```

## Usage

1. **First Launch**: The application will prompt you to download the latest openhash.exe
2. **Configuration**: Set your database path, API port, and P2P port
3. **Start Node**: Click "Start Node" to begin running openhash
4. **Monitor**: Use "View Logs" to see real-time output
5. **Updates**: Click "Check for Updates" to download the latest version

## Application Structure

```
openhash-desktop/
├── src/                    # Frontend source (HTML/CSS/TypeScript)
├── src-tauri/             # Tauri backend (Rust)
├── dist/                  # Built frontend assets
└── src-tauri/target/      # Compiled Rust binaries
```

## Configuration

The application manages the following openhash.exe command:
```bash
./openhash.exe daemon --api-port <API_PORT> --db <DB_PATH> --p2p-port <P2P_PORT>
```

Default values:
- Database Path: `data/data1/node1`
- API Port: `8080`
- P2P Port: `2000`

## Features in Detail

### Auto-Download
- Checks GitHub releases API for the latest version
- Downloads openhash.exe automatically
- Sets proper executable permissions on Unix systems

### Process Management
- Spawns openhash.exe as a child process
- Captures stdout and stderr for logging
- Graceful process termination
- Prevents multiple instances

### Live Logging
- Real-time log streaming with timestamps
- Auto-scrolling to latest entries
- Log history management (keeps last 1000 lines)
- Double-click to clear logs

### State Management
- Visual status indicators (stopped/running/updating)
- Button state management based on process status
- Configuration input validation
- Periodic status checks

## Troubleshooting

### Build Issues
- Ensure all system dependencies are installed
- Try cleaning the build cache: `cd src-tauri && cargo clean`
- Update Rust: `rustup update`

### Runtime Issues
- Check that openhash.exe is downloaded and executable
- Verify port numbers are not in use
- Check database path permissions

## Development

### Adding Features
1. Frontend changes: Edit files in `src/`
2. Backend changes: Edit files in `src-tauri/src/`
3. Test in development: `npm run tauri dev`

### API Commands
The application exposes these Tauri commands:
- `check_executable_exists()`: Check if openhash.exe exists
- `start_node(config)`: Start the openhash node
- `stop_node()`: Stop the running node
- `check_and_download_update()`: Download latest version
- `get_logs()`: Retrieve current logs
- `get_process_status()`: Check if process is running

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]
