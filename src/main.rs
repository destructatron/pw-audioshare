mod application;
mod config;
mod model;
mod pipewire;
mod presets;
mod ui;

use gtk::prelude::*;

fn main() -> glib::ExitCode {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!(
        "Starting {} v{}",
        config::APP_NAME,
        config::VERSION
    );

    // Create and run the application
    let app = application::Application::new();
    app.run()
}
