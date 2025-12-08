use adw::prelude::*;
use adw::subclass::prelude::*;
use async_channel::Receiver;
use gtk::{gio, glib};

use crate::config::APP_ID;
use crate::pipewire::{PipeWireThread, PwEvent};
use crate::ui::Window;

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct Application {
        pub pw_thread: RefCell<Option<PipeWireThread>>,
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
            let window = app.create_window();
            window.present();
        }

        fn startup(&self) {
            self.parent_startup();

            let app = self.obj();

            // Set up application actions
            app.setup_actions();

            // Start PipeWire thread
            app.start_pipewire();
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
            // Dispatch to the active window
            if let Some(window) = self.active_window() {
                if let Some(window) = window.downcast_ref::<Window>() {
                    window.handle_pw_event(event);
                }
            }
        }

        log::debug!("PipeWire event channel closed");
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
