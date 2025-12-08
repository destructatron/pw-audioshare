use glib::Object;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::PortObject)]
    pub struct PortObject {
        #[property(get, set)]
        id: Cell<u32>,

        #[property(get, set)]
        node_id: Cell<u32>,

        #[property(get, set)]
        name: RefCell<String>,

        #[property(get, set)]
        alias: RefCell<String>,

        #[property(get, set)]
        node_name: RefCell<String>,

        #[property(get, set)]
        direction: RefCell<String>,

        #[property(get, set)]
        media_type: RefCell<String>,

        #[property(get, set)]
        channel: RefCell<String>,

        #[property(get, set)]
        display_label: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PortObject {
        const NAME: &'static str = "PwAudiosharePortObject";
        type Type = super::PortObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PortObject {}
}

glib::wrapper! {
    pub struct PortObject(ObjectSubclass<imp::PortObject>);
}

impl PortObject {
    /// Create a new PortObject with all properties
    pub fn new(
        id: u32,
        node_id: u32,
        name: &str,
        alias: Option<&str>,
        node_name: &str,
        direction: &str,
        media_type: &str,
        channel: Option<&str>,
    ) -> Self {
        let port_display = alias.unwrap_or(name);
        let channel_str = channel.unwrap_or("");

        // Create a descriptive label for screen readers
        let display_label = if channel_str.is_empty() {
            format!("{} - {}", node_name, port_display)
        } else {
            format!("{} - {} ({})", node_name, port_display, channel_str)
        };

        Object::builder()
            .property("id", id)
            .property("node-id", node_id)
            .property("name", name)
            .property("alias", alias.unwrap_or(""))
            .property("node-name", node_name)
            .property("direction", direction)
            .property("media-type", media_type)
            .property("channel", channel.unwrap_or(""))
            .property("display-label", &display_label)
            .build()
    }

    /// Check if this is an output port
    pub fn is_output(&self) -> bool {
        self.direction() == "output"
    }

    /// Check if this is an input port
    pub fn is_input(&self) -> bool {
        self.direction() == "input"
    }

    /// Get a detailed description for accessibility
    pub fn accessible_description(&self) -> String {
        let media = self.media_type();
        let dir = if self.is_output() { "output" } else { "input" };
        let channel = self.channel();

        if channel.is_empty() {
            format!("{} {} port on {}", media, dir, self.node_name())
        } else {
            format!(
                "{} {} port, {} channel, on {}",
                media,
                dir,
                channel,
                self.node_name()
            )
        }
    }
}

impl Default for PortObject {
    fn default() -> Self {
        Object::builder().build()
    }
}
