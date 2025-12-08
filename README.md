# PW Audioshare

An accessible GTK4 patchbay for PipeWire. Unlike visual node-graph tools like Helvum, PW Audioshare uses list-based views that work well with screen readers like Orca.

## Features

- Connect and disconnect PipeWire audio, MIDI, and video ports
- Filter ports by type (Audio/MIDI/Video) and search by name
- Bulk connect: select multiple ports and connect them at once
- Save and load connection presets
- Full keyboard navigation
- Screen reader accessible

## Installation

### Dependencies

- GTK4 (4.12+)
- libadwaita (1.4+)
- PipeWire development libraries
- Rust 1.75+

On Fedora:
```bash
sudo dnf install rust cargo gtk4-devel libadwaita-devel pipewire-devel
```

On Debian/Ubuntu:
```bash
sudo apt install rustc cargo libgtk-4-dev libadwaita-1-dev libpipewire-0.3-dev
```

### Building

```bash
cargo build --release
```

The binary will be at `target/release/pw-audioshare`.

## Usage

The interface has three main sections:

1. **Output Ports** (left list) - Sources like microphones, applications, etc.
2. **Input Ports** (right list) - Sinks like speakers, headphones, recorders, etc.
3. **Active Connections** (bottom list) - Currently connected port pairs

### Making Connections

1. Select one or more output ports in the left list
2. Select one or more input ports in the right list
3. Press Ctrl+Enter or click Connect

Connection modes:
- **1 output to N inputs**: Connects to all selected inputs (e.g., mono mic to stereo speakers)
- **N outputs to 1 input**: Connects all outputs to that input (e.g., mixing)
- **N outputs to N inputs**: Connects pairwise by position (e.g., stereo to stereo)

### Keyboard Shortcuts

#### Port Lists (Output/Input)
| Key | Action |
|-----|--------|
| Up/Down | Navigate items |
| Space | Toggle selection |
| Ctrl+A | Select all |
| Left | Move to output list (from input list) |
| Right | Move to input list (from output list) |
| Ctrl+Enter | Connect selected ports |
| Ctrl+Down | Jump to connections list |

#### Connections List
| Key | Action |
|-----|--------|
| Up/Down | Navigate connections |
| Delete/Backspace | Delete selected connection |
| Ctrl+Up | Return to previous port list |

### Filtering

Use the search box to filter ports by name. Toggle the Audio, MIDI, and Video buttons to show/hide port types.

### Presets

Save your current connections as a preset to quickly restore them later:

1. Click the preset menu button (floppy disk icon) in the header
2. Select "Save Preset..." and enter a name
3. To restore, select "Load Preset..." and choose from the list

Presets are saved by node and port names, so they work across sessions even if port IDs change.

Preset file location: `~/.config/pw-audioshare/presets.json`

## License

MIT
