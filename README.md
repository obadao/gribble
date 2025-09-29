# Gribble System Monitor

A real-time terminal-based system monitor built with Rust and ratatui. Gribble provides live system metrics, process management, file browsing, and network traffic visualization in a keyboard-driven interface.

![Gribble System Monitor](screenshot.png)

## Features

- **System Monitoring**: Real-time CPU usage with progress bars, memory statistics, system uptime, and architecture details
- **Process Management**: Interactive process viewer with CPU and memory usage, sortable by resource consumption
- **File Explorer**: Navigate filesystem with keyboard controls, directory traversal with visual indicators
- **Network Traffic**: Live network monitoring with sparkline graphs showing download/upload rates and totals
- **Keyboard Navigation**: Vim-style navigation (hjkl) plus arrow keys for intuitive control

## Installation

### Prerequisites

- Rust 1.70+ (2024 edition)
- Terminal with Unicode support

### Build from Source

```bash
git clone https://github.com/Cod-e-Codes/gribble.git
cd gribble
cargo build --release
```

### Run

```bash
cargo run
```

## Usage

### Navigation

- `←→` or `h l` - Switch between panels  
- `↑↓` or `j k` - Navigate within Process Manager and File Explorer
- `Enter` - Open directories in File Explorer
- `r` - Refresh all data
- `?` - Show/hide help
- `q` or `Esc` - Quit

### Panels

1. **System Monitor** - CPU usage with visual bars, memory statistics, process count, system information
2. **System Status** - Current time/date, disk usage, network interface statistics, system load
3. **Process Manager** - Live process list sorted by CPU usage, shows memory consumption, navigable selection
4. **File Explorer** - Directory browser with folder/file icons, supports navigation up and into directories
5. **Network Graph** - Real-time network traffic visualization with separate RX/TX sparkline graphs

## Technical Details

- Built with ratatui for terminal UI rendering
- Uses sysinfo for cross-platform system metrics
- Implements proper scrolling for long lists
- Updates system data every 2 seconds
- Maintains 60-point history for network graphs (2 minutes of data)
- Cross-platform support (Windows, macOS, Linux)

## Requirements

- Modern terminal emulator with Unicode support
- Sufficient terminal size (minimum 80x24 recommended)
- Read permissions for system information and process data

## License

MIT License - see LICENSE file for details

## Contributing

Pull requests welcome. Please ensure code follows Rust formatting standards and includes appropriate error handling.
