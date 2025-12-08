use glib::Object;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::{Cell, RefCell};

mod imp {
    use super::*;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::LinkObject)]
    pub struct LinkObject {
        #[property(get, set)]
        id: Cell<u32>,

        #[property(get, set)]
        output_port_id: Cell<u32>,

        #[property(get, set)]
        input_port_id: Cell<u32>,

        #[property(get, set)]
        output_label: RefCell<String>,

        #[property(get, set)]
        input_label: RefCell<String>,

        #[property(get, set)]
        state: RefCell<String>,

        #[property(get, set)]
        display_label: RefCell<String>,

        #[property(get, set)]
        media_type: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LinkObject {
        const NAME: &'static str = "PwAudioshareLinkObject";
        type Type = super::LinkObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for LinkObject {}
}

glib::wrapper! {
    pub struct LinkObject(ObjectSubclass<imp::LinkObject>);
}

impl LinkObject {
    /// Create a new LinkObject with all properties
    pub fn new(
        id: u32,
        output_port_id: u32,
        input_port_id: u32,
        output_label: &str,
        input_label: &str,
        state: &str,
        media_type: &str,
    ) -> Self {
        let display_label = format!("{} -> {}", output_label, input_label);

        Object::builder()
            .property("id", id)
            .property("output-port-id", output_port_id)
            .property("input-port-id", input_port_id)
            .property("output-label", output_label)
            .property("input-label", input_label)
            .property("state", state)
            .property("display-label", &display_label)
            .property("media-type", media_type)
            .build()
    }

    /// Check if the link is active
    pub fn is_active(&self) -> bool {
        self.state() == "active"
    }

    /// Get a detailed description for accessibility
    pub fn accessible_description(&self) -> String {
        let state_desc = match self.state().as_str() {
            "active" => "active",
            "paused" => "paused",
            "error" => "error state",
            _ => "unknown state",
        };

        format!(
            "{} connection from {} to {}, {}",
            self.media_type(),
            self.output_label(),
            self.input_label(),
            state_desc
        )
    }
}

impl Default for LinkObject {
    fn default() -> Self {
        Object::builder().build()
    }
}
