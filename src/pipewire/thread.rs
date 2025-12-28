use std::cell::RefCell;
use std::rc::Rc;
use std::thread::{self, JoinHandle};

use async_channel::{Receiver, Sender};
use pipewire::context::Context;
use pipewire::core::Core;
use pipewire::link::Link;
use pipewire::main_loop::MainLoop;
use pipewire::registry::GlobalObject;
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;

use super::messages::{LinkState, MediaType, PortDirection, PwEvent, UiCommand};

/// Manages the PipeWire connection running in a separate thread
pub struct PipeWireThread {
    handle: Option<JoinHandle<()>>,
    command_tx: Sender<UiCommand>,
}

impl PipeWireThread {
    /// Spawn a new PipeWire thread that sends events to the given sender
    pub fn spawn(event_tx: Sender<PwEvent>) -> Result<Self, anyhow::Error> {
        let (command_tx, command_rx) = async_channel::bounded::<UiCommand>(64);

        let handle = thread::Builder::new()
            .name("pipewire".into())
            .spawn(move || {
                if let Err(e) = run_pipewire_loop(event_tx.clone(), command_rx) {
                    log::error!("PipeWire thread error: {}", e);
                    let _ = event_tx.send_blocking(PwEvent::Disconnected {
                        reason: e.to_string(),
                    });
                }
            })?;

        Ok(Self {
            handle: Some(handle),
            command_tx,
        })
    }

    /// Get a sender to send commands to the PipeWire thread
    pub fn command_sender(&self) -> Sender<UiCommand> {
        self.command_tx.clone()
    }

    /// Request shutdown and wait for the thread to finish
    pub fn shutdown(&mut self) {
        let _ = self.command_tx.send_blocking(UiCommand::Quit);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PipeWireThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// State shared within the PipeWire thread
struct ThreadState {
    event_tx: Sender<PwEvent>,
    core: Core,
    /// Store created links to keep them alive without leaking memory.
    /// The `object.linger = true` property ensures PipeWire keeps the connection
    /// even after the proxy is dropped, but we need to keep the proxy alive
    /// while the app is running.
    created_links: Vec<Link>,
}

/// Run the PipeWire main loop
fn run_pipewire_loop(
    event_tx: Sender<PwEvent>,
    command_rx: Receiver<UiCommand>,
) -> Result<(), anyhow::Error> {
    // Initialize PipeWire
    pipewire::init();

    let mainloop = MainLoop::new(None)?;
    let context = Context::new(&mainloop)?;
    let core = context.connect(None)?;
    let registry = core.get_registry()?;

    // Shared state for callbacks
    let state = Rc::new(RefCell::new(ThreadState {
        event_tx: event_tx.clone(),
        core: core.clone(),
        created_links: Vec::new(),
    }));

    // Set up registry listener for global object events
    let state_clone = state.clone();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            handle_global_added(&state_clone.borrow().event_tx, global);
        })
        .global_remove({
            let event_tx = event_tx.clone();
            move |id| {
                handle_global_removed(&event_tx, id);
            }
        })
        .register();

    // Notify that we're connected
    let _ = event_tx.send_blocking(PwEvent::Connected);

    // Set up a receiver for UI commands using the main loop
    let mainloop_weak = mainloop.downgrade();
    let state_for_commands = state.clone();
    let event_tx_for_commands = event_tx.clone();

    // Use a timer to poll for commands (pipewire-rs doesn't have direct channel integration)
    let _timer = mainloop.loop_().add_timer(move |_| {
        // Process all pending commands
        while let Ok(cmd) = command_rx.try_recv() {
            match cmd {
                UiCommand::CreateLink {
                    output_port_id,
                    input_port_id,
                } => {
                    if let Err(e) = handle_create_link(
                        &mut state_for_commands.borrow_mut(),
                        output_port_id,
                        input_port_id,
                    ) {
                        log::error!("Failed to create link: {}", e);
                        let _ = event_tx_for_commands.send_blocking(PwEvent::Error {
                            message: format!("Failed to create connection: {}", e),
                        });
                    }
                }
                UiCommand::DeleteLink { link_id } => {
                    if let Err(e) = handle_delete_link(&state_for_commands.borrow(), link_id) {
                        log::error!("Failed to delete link: {}", e);
                        let _ = event_tx_for_commands.send_blocking(PwEvent::Error {
                            message: format!("Failed to delete connection: {}", e),
                        });
                    }
                }
                UiCommand::Quit => {
                    if let Some(mainloop) = mainloop_weak.upgrade() {
                        mainloop.quit();
                    }
                    return;
                }
            }
        }
    });

    // Start the timer to fire every 50ms
    _timer.update_timer(
        Some(std::time::Duration::from_millis(50)),
        Some(std::time::Duration::from_millis(50)),
    );

    // Run the main loop
    mainloop.run();

    Ok(())
}

/// Handle a new global object appearing in the registry
fn handle_global_added<T>(tx: &Sender<PwEvent>, global: &GlobalObject<T>)
where
    T: AsRef<DictRef>,
{
    let props = match global.props.as_ref() {
        Some(p) => p.as_ref(),
        None => return,
    };

    match global.type_ {
        ObjectType::Node => {
            let event = PwEvent::NodeAdded {
                id: global.id,
                name: props.get("node.name").unwrap_or("Unknown").to_string(),
                media_class: props.get("media.class").map(String::from),
                description: props.get("node.description").map(String::from),
                application_name: props.get("application.name").map(String::from),
            };
            let _ = tx.send_blocking(event);
        }
        ObjectType::Port => {
            let direction = match props.get("port.direction") {
                Some("in") => PortDirection::Input,
                Some("out") => PortDirection::Output,
                _ => return, // Skip ports with unknown direction
            };

            let media_type = MediaType::from_format_dsp(props.get("format.dsp"));

            let event = PwEvent::PortAdded {
                id: global.id,
                node_id: props
                    .get("node.id")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                name: props.get("port.name").unwrap_or("Unknown").to_string(),
                alias: props.get("port.alias").map(String::from),
                direction,
                media_type,
                channel: props.get("audio.channel").map(String::from),
            };
            let _ = tx.send_blocking(event);
        }
        ObjectType::Link => {
            let event = PwEvent::LinkAdded {
                id: global.id,
                output_node_id: props
                    .get("link.output.node")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                output_port_id: props
                    .get("link.output.port")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                input_node_id: props
                    .get("link.input.node")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                input_port_id: props
                    .get("link.input.port")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                state: LinkState::Active,
            };
            let _ = tx.send_blocking(event);
        }
        _ => {}
    }
}

/// Handle a global object being removed from the registry
fn handle_global_removed(tx: &Sender<PwEvent>, id: u32) {
    // We don't know what type was removed, so send all possible removals
    // The UI will ignore removals for IDs it doesn't know about
    let _ = tx.send_blocking(PwEvent::NodeRemoved { id });
    let _ = tx.send_blocking(PwEvent::PortRemoved { id });
    let _ = tx.send_blocking(PwEvent::LinkRemoved { id });
}

/// Create a link between two ports
fn handle_create_link(
    state: &mut ThreadState,
    output_port_id: u32,
    input_port_id: u32,
) -> Result<(), anyhow::Error> {
    // Create properties for the link
    let props = pipewire::properties::properties! {
        "link.output.port" => output_port_id.to_string(),
        "link.input.port" => input_port_id.to_string(),
        "object.linger" => "true",
    };

    // Create the link using the core
    let link: Link = state.core.create_object("link-factory", &props)?;

    // Store the link to keep it alive. When ThreadState is dropped during
    // shutdown, links will be properly cleaned up.
    state.created_links.push(link);

    Ok(())
}

/// Delete an existing link by ID
/// Note: This is a simplified implementation. In a production app, you'd want to
/// keep track of link proxies or use pw-link command as a fallback.
fn handle_delete_link(_state: &ThreadState, link_id: u32) -> Result<(), anyhow::Error> {
    // Use pw-link command to delete the link as a workaround
    // The pipewire-rs API requires a GlobalObject to bind, which we don't have here
    let output = std::process::Command::new("pw-link")
        .args(["-d", &link_id.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to delete link {}: {}", link_id, stderr);
    }

    Ok(())
}
