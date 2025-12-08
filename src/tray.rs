use ksni::TrayMethods;
use std::sync::mpsc;
use std::thread;

/// Commands sent from the system tray to the main application
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Show the main window
    Show,
    /// Quit the application
    Quit,
}

/// Handle to communicate with the tray
pub struct TrayHandle {
    _thread: thread::JoinHandle<()>,
}

struct PwAudioshareTray {
    command_tx: mpsc::Sender<TrayCommand>,
    active_preset: Option<String>,
}

impl ksni::Tray for PwAudioshareTray {
    fn id(&self) -> String {
        "pw-audioshare".into()
    }

    fn icon_name(&self) -> String {
        // Use a standard audio icon
        "audio-card".into()
    }

    fn title(&self) -> String {
        match &self.active_preset {
            Some(name) => format!("PW Audioshare [{}]", name),
            None => "PW Audioshare".into(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let mut items = vec![
            StandardItem {
                label: "Show PW Audioshare".into(),
                icon_name: "window-new".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.command_tx.send(TrayCommand::Show);
                }),
                ..Default::default()
            }
            .into(),
        ];

        // Show active preset status if one is active
        if let Some(ref name) = self.active_preset {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: format!("Active: {}", name),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.command_tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

/// Spawn the system tray in a background thread
/// Returns a receiver for tray commands and a handle to keep the tray alive
pub fn spawn_tray(active_preset: Option<String>) -> (mpsc::Receiver<TrayCommand>, TrayHandle) {
    let (command_tx, command_rx) = mpsc::channel();

    let thread = thread::spawn(move || {
        // Create a new Tokio runtime for this thread
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime for tray");

        rt.block_on(async {
            let tray = PwAudioshareTray {
                command_tx,
                active_preset,
            };

            match tray.spawn().await {
                Ok(_handle) => {
                    // Keep the tray alive forever
                    std::future::pending::<()>().await;
                }
                Err(e) => {
                    log::error!("Failed to spawn system tray: {}", e);
                }
            }
        });
    });

    (command_rx, TrayHandle { _thread: thread })
}
