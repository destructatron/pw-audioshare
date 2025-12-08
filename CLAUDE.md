# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PW-Audioshare is an accessible GTK4 patchbay for PipeWire, designed for screen reader users. Unlike visual node-graph tools like Helvum, it uses list-based views with proper accessibility attributes.

## Build Commands

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo run            # Run the application (requires PipeWire running)
RUST_LOG=debug cargo run  # Run with debug logging
```

## Architecture

### Thread Model

The application uses two threads communicating via async channels:

1. **GTK Main Thread** - Handles UI, runs the GLib main loop
2. **PipeWire Thread** - Runs PipeWire's MainLoop, handles registry events

Communication flows:
- `PwEvent` (pipewire → UI): Node/port/link added/removed events
- `UiCommand` (UI → pipewire): CreateLink, DeleteLink, Quit

### Module Structure

- **`application.rs`** - AdwApplication subclass, spawns PipeWire thread, routes events to window
- **`pipewire/thread.rs`** - PipeWire MainLoop, registry listener, link creation/deletion
- **`pipewire/messages.rs`** - `PwEvent` and `UiCommand` enums for cross-thread messaging
- **`pipewire/state.rs`** - Internal state structs (PwNode, PwPort, PwLink)
- **`model/port_object.rs`** - GObject wrapper for ports (required for GTK ListStore)
- **`model/link_object.rs`** - GObject wrapper for links
- **`ui/window.rs`** - Main window with dual-list UI, handles PwEvents

### GObject Pattern

GTK4-rs requires GObject wrappers for data in ListStore. Each wrapper uses:
- `imp` module with `#[derive(glib::Properties)]` for property definitions
- `glib::wrapper!` macro for the public type
- `ObjectSubclass` impl for GLib type registration

### Key Accessibility Features

- All interactive elements have programmatic labels via tooltips
- List items announce: "NodeName - PortName (channel)"
- Full keyboard navigation: Tab between panels, arrows within lists, Enter to connect

## Dependencies Note

Requires system libraries: GTK4 (4.12+), libadwaita (1.4+), PipeWire development files.
