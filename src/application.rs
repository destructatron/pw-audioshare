use adw::prelude::*;
use adw::subclass::prelude::*;
use async_channel::Receiver;
use gtk::{gio, glib};
use std::sync::mpsc;

use crate::config::APP_ID;
use crate::pipewire::{PipeWireThread, PwEvent};
use crate::presets::PresetStore;
use crate::settings::Settings;
use crate::tray::{self, TrayCommand, TrayHandle};
use crate::ui::Window;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    pub struct Application {
        pub pw_thread: RefCell<Option<PipeWireThread>>,
        pub tray_handle: RefCell<Option<TrayHandle>>,
        pub tray_rx: RefCell<Option<mpsc::Receiver<TrayCommand>>>,
        /// Track if this is the first activation (startup)
        pub first_activation: Cell<bool>,
    }

    impl Default for Application {
        fn default() -> Self {
            Self {
                pw_thread: RefCell::new(None),
                tray_handle: RefCell::new(None),
                tray_rx: RefCell::new(None),
                first_activation: Cell::new(true),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "PwAudioshareApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            let app = self.obj();

            // Check if this is the first activation (startup)
            let is_first = self.first_activation.get();
            if is_first {
                self.first_activation.set(false);

                // Check if we should start minimized
                let settings = Settings::load();
                if settings.start_minimized {
                    log::info!("Starting minimized to tray");
                    // Create window but don't show it
                    let _window = app.create_window();
                    // Window is created but not presented - will be shown via tray
                    return;
                }
            }

            // Normal activation: show the window
            if let Some(window) = app.active_window() {
                window.set_visible(true);
                window.present();
            } else {
                let window = app.create_window();
                window.present();
            }
        }

        fn startup(&self) {
            self.parent_startup();

            let app = self.obj();

            // Set up application actions
            app.setup_actions();

            // Start PipeWire thread
            app.start_pipewire();

            // Start system tray
            app.start_tray();
        }

        fn shutdown(&self) {
            // Stop PipeWire thread
            if let Some(mut thread) = self.pw_thread.take() {
                thread.shutdown();
            }

            self.parent_shutdown();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", APP_ID)
            .property("flags", gio::ApplicationFlags::FLAGS_NONE)
            .build()
    }

    /// Create the main window
    fn create_window(&self) -> Window {
        let window = Window::new(self.upcast_ref());

        // Give the window the command sender
        if let Some(thread) = self.imp().pw_thread.borrow().as_ref() {
            window.set_command_sender(thread.command_sender());
        }

        // Override close-request to minimize to tray instead of quitting
        window.connect_close_request(|window| {
            // Hide the window instead of closing
            window.set_visible(false);
            // Stop the event from propagating (prevents actual close)
            glib::Propagation::Stop
        });

        window
    }

    /// Set up application-level actions
    fn setup_actions(&self) {
        // Quit action
        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate(glib::clone!(
            #[weak(rename_to = app)]
            self,
            move |_, _| {
                app.quit();
            }
        ));
        self.add_action(&action_quit);

        // Set up keyboard shortcuts
        self.set_accels_for_action("app.quit", &["<Ctrl>q"]);
        self.set_accels_for_action("win.connect-selected", &["<Ctrl>Return"]);
    }

    /// Start the PipeWire thread and set up event handling
    fn start_pipewire(&self) {
        let (event_tx, event_rx) = async_channel::unbounded::<PwEvent>();

        // Start the PipeWire thread
        match PipeWireThread::spawn(event_tx) {
            Ok(thread) => {
                self.imp().pw_thread.replace(Some(thread));
                log::info!("PipeWire thread started");
            }
            Err(e) => {
                log::error!("Failed to start PipeWire thread: {}", e);
                return;
            }
        }

        // Set up event receiver on GTK main loop
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = app)]
            self,
            async move {
                app.process_pw_events(event_rx).await;
            }
        ));
    }

    /// Process events from PipeWire thread
    async fn process_pw_events(&self, rx: Receiver<PwEvent>) {
        while let Ok(event) = rx.recv().await {
            // Get any window, not just the "active" one.
            // active_window() returns None when the window is hidden (e.g., minimized to tray),
            // but windows() returns all toplevel windows regardless of visibility.
            if let Some(window) = self.windows().into_iter().next() {
                if let Some(window) = window.downcast_ref::<Window>() {
                    window.handle_pw_event(event);
                }
            }
        }

        log::debug!("PipeWire event channel closed");
    }

    /// Start the system tray
    fn start_tray(&self) {
        // Get active preset name to show in tray
        let active_preset = PresetStore::load().active_preset;

        // Spawn tray in background thread
        let (tray_rx, tray_handle) = tray::spawn_tray(active_preset);

        self.imp().tray_handle.replace(Some(tray_handle));
        self.imp().tray_rx.replace(Some(tray_rx));

        log::info!("System tray started");

        // Set up polling for tray commands on GTK main loop
        glib::timeout_add_local(
            std::time::Duration::from_millis(100),
            glib::clone!(
                #[weak(rename_to = app)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    app.process_tray_commands();
                    glib::ControlFlow::Continue
                }
            ),
        );
    }

    /// Process pending tray commands
    fn process_tray_commands(&self) {
        let rx = self.imp().tray_rx.borrow();
        if let Some(rx) = rx.as_ref() {
            // Process all pending commands (non-blocking)
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    TrayCommand::Show => {
                        log::debug!("Tray: Show window");
                        if let Some(window) = self.active_window() {
                            window.set_visible(true);
                            window.present();
                        } else {
                            // No window exists, create one
                            let window = self.create_window();
                            window.present();
                        }
                    }
                    TrayCommand::Quit => {
                        log::debug!("Tray: Quit application");
                        self.quit();
                    }
                }
            }
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
