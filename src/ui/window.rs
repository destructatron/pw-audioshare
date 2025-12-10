use std::cell::RefCell;
use std::collections::HashSet;

use adw::prelude::*;
use adw::subclass::prelude::*;
use async_channel::Sender;
use gtk::gdk::Key;
use gtk::glib::Propagation;
use gtk::{gio, glib};

use crate::model::{LinkObject, PortObject};
use crate::pipewire::{PortDirection, PwEvent, PwState, UiCommand};
use crate::presets::{Preset, PresetConnection, PresetStore};
use crate::settings::Settings;

mod imp {
    use super::*;

    #[derive(gtk::CompositeTemplate)]
    #[template(string = r#"
        <interface>
            <template class="PwAudioshareWindow" parent="AdwApplicationWindow">
                <property name="title">PW Audioshare</property>
                <property name="default-width">900</property>
                <property name="default-height">700</property>
                <child>
                    <object class="GtkBox" id="main_box">
                        <property name="orientation">vertical</property>
                        <child>
                            <object class="AdwHeaderBar">
                                <property name="title-widget">
                                    <object class="AdwWindowTitle">
                                        <property name="title">PW Audioshare</property>
                                        <property name="subtitle">PipeWire Patchbay</property>
                                    </object>
                                </property>
                                <child type="end">
                                    <object class="GtkMenuButton" id="preset_menu_button">
                                        <property name="icon-name">document-save-symbolic</property>
                                        <property name="tooltip-text">Presets</property>
                                        <property name="menu-model">preset_menu</property>
                                    </object>
                                </child>
                            </object>
                        </child>
                    </object>
                </child>
            </template>
            <menu id="preset_menu">
                <section>
                    <item>
                        <attribute name="label">Save Preset...</attribute>
                        <attribute name="action">win.save-preset</attribute>
                    </item>
                    <item>
                        <attribute name="label">Manage Presets...</attribute>
                        <attribute name="action">win.load-preset</attribute>
                    </item>
                </section>
                <section>
                    <item>
                        <attribute name="label">Deactivate Auto-connect</attribute>
                        <attribute name="action">win.deactivate-preset</attribute>
                    </item>
                </section>
                <section>
                    <item>
                        <attribute name="label">Start Minimized to Tray</attribute>
                        <attribute name="action">win.start-minimized</attribute>
                    </item>
                </section>
            </menu>
        </interface>
    "#)]
    pub struct Window {
        #[template_child]
        pub main_box: TemplateChild<gtk::Box>,

        // Data models
        pub output_ports: gio::ListStore,
        pub input_ports: gio::ListStore,
        pub links: gio::ListStore,

        // PipeWire state tracking
        pub pw_state: RefCell<PwState>,

        // Command sender for PipeWire thread
        pub command_tx: RefCell<Option<Sender<UiCommand>>>,

        // Filter state
        pub search_text: RefCell<String>,
        pub show_audio: RefCell<bool>,
        pub show_midi: RefCell<bool>,
        pub show_video: RefCell<bool>,

        // Widget references (MultiSelection for bulk connect)
        pub output_selection: RefCell<Option<gtk::MultiSelection>>,
        pub input_selection: RefCell<Option<gtk::MultiSelection>>,
        pub output_list_view: RefCell<Option<gtk::ListView>>,
        pub input_list_view: RefCell<Option<gtk::ListView>>,
        pub connections_list_view: RefCell<Option<gtk::ListView>>,
        pub connections_selection: RefCell<Option<gtk::SingleSelection>>,
        pub status_label: RefCell<Option<gtk::Label>>,

        // Filter references
        pub output_filter: RefCell<Option<gtk::CustomFilter>>,
        pub input_filter: RefCell<Option<gtk::CustomFilter>>,

        // Track which port list was last focused (true = output, false = input)
        pub last_port_list_was_output: RefCell<bool>,

        // Track pending delete position for selection preservation
        pub pending_delete_position: RefCell<Option<u32>>,

        // Preset storage
        pub preset_store: RefCell<PresetStore>,

        // Track in-flight link creation requests to prevent duplicates
        // Key is (output_port_id, input_port_id)
        pub pending_links: RefCell<HashSet<(u32, u32)>>,

        // Application settings
        pub settings: RefCell<Settings>,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                main_box: TemplateChild::default(),
                output_ports: gio::ListStore::new::<PortObject>(),
                input_ports: gio::ListStore::new::<PortObject>(),
                links: gio::ListStore::new::<LinkObject>(),
                pw_state: RefCell::new(PwState::new()),
                command_tx: RefCell::new(None),
                search_text: RefCell::new(String::new()),
                show_audio: RefCell::new(true),
                show_midi: RefCell::new(true),
                show_video: RefCell::new(true),
                output_selection: RefCell::new(None),
                input_selection: RefCell::new(None),
                output_list_view: RefCell::new(None),
                input_list_view: RefCell::new(None),
                connections_list_view: RefCell::new(None),
                connections_selection: RefCell::new(None),
                status_label: RefCell::new(None),
                output_filter: RefCell::new(None),
                input_filter: RefCell::new(None),
                last_port_list_was_output: RefCell::new(true),
                pending_delete_position: RefCell::new(None),
                preset_store: RefCell::new(PresetStore::load()),
                pending_links: RefCell::new(HashSet::new()),
                settings: RefCell::new(Settings::load()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "PwAudioshareWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_ui();
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Accessible;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    /// Set the command sender for PipeWire communication
    pub fn set_command_sender(&self, tx: Sender<UiCommand>) {
        self.imp().command_tx.replace(Some(tx));
    }

    /// Handle a PipeWire event
    pub fn handle_pw_event(&self, event: PwEvent) {
        match event {
            PwEvent::Connected => {
                self.update_status("Connected to PipeWire", false);
            }
            PwEvent::Disconnected { reason } => {
                self.update_status(&format!("Disconnected: {}", reason), false);
            }
            PwEvent::NodeAdded {
                id,
                name,
                media_class,
                description,
                application_name,
            } => {
                let mut state = self.imp().pw_state.borrow_mut();
                state.nodes.insert(
                    id,
                    crate::pipewire::state::PwNode {
                        id,
                        name,
                        media_class,
                        description,
                        application_name,
                    },
                );
            }
            PwEvent::NodeRemoved { id } => {
                self.imp().pw_state.borrow_mut().nodes.remove(&id);
            }
            PwEvent::PortAdded {
                id,
                node_id,
                name,
                alias,
                direction,
                media_type,
                channel,
            } => {
                // Determine actual media type - if Unknown, check the node's media.class
                let actual_media_type = {
                    let state = self.imp().pw_state.borrow();
                    if media_type == crate::pipewire::messages::MediaType::Unknown {
                        // Try to infer from node's media.class
                        state.nodes.get(&node_id).map(|n| {
                            if let Some(ref mc) = n.media_class {
                                let mc_lower = mc.to_lowercase();
                                if mc_lower.contains("video") {
                                    crate::pipewire::messages::MediaType::Video
                                } else if mc_lower.contains("midi") {
                                    crate::pipewire::messages::MediaType::Midi
                                } else if mc_lower.contains("audio") || mc_lower.contains("stream") {
                                    crate::pipewire::messages::MediaType::Audio
                                } else {
                                    media_type
                                }
                            } else {
                                media_type
                            }
                        }).unwrap_or(media_type)
                    } else {
                        media_type
                    }
                };

                // Store in PW state
                {
                    let mut state = self.imp().pw_state.borrow_mut();
                    state.ports.insert(
                        id,
                        crate::pipewire::state::PwPort {
                            id,
                            node_id,
                            name: name.clone(),
                            alias: alias.clone(),
                            direction,
                            media_type: actual_media_type,
                            channel: channel.clone(),
                        },
                    );
                }

                // Get node name
                let node_name = {
                    let state = self.imp().pw_state.borrow();
                    state
                        .nodes
                        .get(&node_id)
                        .map(|n| n.display_name().to_string())
                        .unwrap_or_else(|| format!("Node {}", node_id))
                };

                // Create GObject and add to appropriate list
                let port_obj = PortObject::new(
                    id,
                    node_id,
                    &name,
                    alias.as_deref(),
                    &node_name,
                    direction.as_str(),
                    actual_media_type.as_str(),
                    channel.as_deref(),
                );

                match direction {
                    PortDirection::Output => {
                        self.imp().output_ports.append(&port_obj);
                    }
                    PortDirection::Input => {
                        self.imp().input_ports.append(&port_obj);
                    }
                }

                self.update_status_counts();

                // Check if this new port completes any auto-connect preset connections
                self.check_auto_connect();
            }
            PwEvent::PortRemoved { id } => {
                self.imp().pw_state.borrow_mut().ports.remove(&id);
                self.remove_port_from_lists(id);
                self.update_status_counts();
            }
            PwEvent::LinkAdded {
                id,
                output_node_id: _,
                output_port_id,
                input_node_id: _,
                input_port_id,
                state,
            } => {
                // Store in PW state
                {
                    let mut pw_state = self.imp().pw_state.borrow_mut();
                    pw_state.links.insert(
                        id,
                        crate::pipewire::state::PwLink {
                            id,
                            output_node_id: 0,
                            output_port_id,
                            input_node_id: 0,
                            input_port_id,
                            state,
                        },
                    );
                }

                // Remove from pending links (link creation confirmed)
                self.imp()
                    .pending_links
                    .borrow_mut()
                    .remove(&(output_port_id, input_port_id));

                // Get labels for the link
                let (output_label, input_label, media_type) = {
                    let pw_state = self.imp().pw_state.borrow();
                    let out_label = pw_state
                        .ports
                        .get(&output_port_id)
                        .and_then(|p| {
                            let node = pw_state.nodes.get(&p.node_id)?;
                            Some(format!("{} - {}", node.display_name(), p.display_name()))
                        })
                        .unwrap_or_else(|| format!("Port {}", output_port_id));

                    let in_label = pw_state
                        .ports
                        .get(&input_port_id)
                        .and_then(|p| {
                            let node = pw_state.nodes.get(&p.node_id)?;
                            Some(format!("{} - {}", node.display_name(), p.display_name()))
                        })
                        .unwrap_or_else(|| format!("Port {}", input_port_id));

                    let media = pw_state
                        .ports
                        .get(&output_port_id)
                        .map(|p| p.media_type.as_str())
                        .unwrap_or("unknown");

                    (out_label, in_label, media.to_string())
                };

                let link_obj = LinkObject::new(
                    id,
                    output_port_id,
                    input_port_id,
                    &output_label,
                    &input_label,
                    state.as_str(),
                    &media_type,
                );

                self.imp().links.append(&link_obj);
                self.update_status_counts();
            }
            PwEvent::LinkRemoved { id } => {
                // Get port IDs before removing from state (to clean up pending_links)
                let port_ids = {
                    let pw_state = self.imp().pw_state.borrow();
                    pw_state
                        .links
                        .get(&id)
                        .map(|l| (l.output_port_id, l.input_port_id))
                };

                // Clean up pending_links if this link was pending
                if let Some(key) = port_ids {
                    self.imp().pending_links.borrow_mut().remove(&key);
                }

                self.imp().pw_state.borrow_mut().links.remove(&id);
                self.remove_link_from_list(id);
                self.update_status_counts();
            }
            PwEvent::LinkStateChanged { id, state } => {
                // Update link state in model
                for i in 0..self.imp().links.n_items() {
                    if let Some(link) = self.imp().links.item(i).and_downcast::<LinkObject>() {
                        if link.id() == id {
                            link.set_state(state.as_str());
                            break;
                        }
                    }
                }
            }
            PwEvent::Error { message } => {
                log::error!("PipeWire error: {}", message);
                self.update_status(&format!("Error: {}", message), false);
                self.announce(&message);
            }
        }
    }

    /// Set up the complete UI
    fn setup_ui(&self) {
        let imp = self.imp();
        let main_box = &*imp.main_box;

        // Create filter bar
        let filter_bar = self.build_filter_bar();
        main_box.append(&filter_bar);

        // Create main content area with port lists
        let content = self.build_content_area();
        main_box.append(&content);

        // Create connections panel
        let connections = self.build_connections_panel();
        main_box.append(&connections);

        // Create status bar
        let status_bar = self.build_status_bar();
        main_box.append(&status_bar);

        // Setup actions
        self.setup_actions();

        // Show active preset if one was saved from previous session
        self.update_active_preset_display();
    }

    /// Build the filter bar with search and media type toggles
    fn build_filter_bar(&self) -> gtk::Box {
        let bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .accessible_role(gtk::AccessibleRole::Toolbar)
            .build();

        // Search entry
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search ports...")
            .hexpand(true)
            .tooltip_text("Filter ports by name")
            .build();

        // Connect search
        search.connect_search_changed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |entry| {
                let text = entry.text().to_string();
                window.imp().search_text.replace(text);
                window.apply_filters();
            }
        ));

        bar.append(&search);

        // Media type toggles
        let audio_btn = gtk::ToggleButton::builder()
            .label("Audio")
            .active(true)
            .tooltip_text("Show audio ports")
            .build();

        let midi_btn = gtk::ToggleButton::builder()
            .label("MIDI")
            .active(true)
            .tooltip_text("Show MIDI ports")
            .build();

        let video_btn = gtk::ToggleButton::builder()
            .label("Video")
            .active(true)
            .tooltip_text("Show video ports")
            .build();

        // Connect toggles
        audio_btn.connect_toggled(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |btn| {
                window.imp().show_audio.replace(btn.is_active());
                window.apply_filters();
            }
        ));

        midi_btn.connect_toggled(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |btn| {
                window.imp().show_midi.replace(btn.is_active());
                window.apply_filters();
            }
        ));

        video_btn.connect_toggled(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |btn| {
                window.imp().show_video.replace(btn.is_active());
                window.apply_filters();
            }
        ));

        bar.append(&audio_btn);
        bar.append(&midi_btn);
        bar.append(&video_btn);

        bar
    }

    /// Build the main content area with output and input port lists
    fn build_content_area(&self) -> gtk::Box {
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .homogeneous(true)
            .vexpand(true)
            .build();

        // Output ports panel
        let output_panel = self.build_port_panel("Output Ports (Sources)", true);
        content.append(&output_panel);

        // Input ports panel
        let input_panel = self.build_port_panel("Input Ports (Sinks)", false);
        content.append(&input_panel);

        content
    }

    /// Build a port list panel (either outputs or inputs)
    fn build_port_panel(&self, title: &str, is_output: bool) -> gtk::Frame {
        let frame = gtk::Frame::builder().label(title).build();

        let panel_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_start(6)
            .margin_end(6)
            .margin_top(6)
            .margin_bottom(6)
            .build();

        // Get the appropriate model
        let model = if is_output {
            self.imp().output_ports.clone()
        } else {
            self.imp().input_ports.clone()
        };

        // Create filter model
        let filter = gtk::CustomFilter::new(|_| true);
        let filter_model = gtk::FilterListModel::new(Some(model), Some(filter.clone()));

        // Store filter reference for later updates
        if is_output {
            self.imp().output_filter.replace(Some(filter));
        } else {
            self.imp().input_filter.replace(Some(filter));
        }

        // Create sort model (sort by display label)
        let sorter = gtk::CustomSorter::new(|a, b| {
            let port_a = a.downcast_ref::<PortObject>().unwrap();
            let port_b = b.downcast_ref::<PortObject>().unwrap();
            port_a.display_label().cmp(&port_b.display_label()).into()
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter));

        // Selection model (MultiSelection for bulk connect)
        let selection = gtk::MultiSelection::new(Some(sort_model));

        // Store selection reference
        if is_output {
            self.imp().output_selection.replace(Some(selection.clone()));
        } else {
            self.imp().input_selection.replace(Some(selection.clone()));
        }

        // Factory for list items
        let factory = gtk::SignalListItemFactory::new();

        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::builder()
                .halign(gtk::Align::Start)
                .xalign(0.0)
                .margin_start(6)
                .margin_end(6)
                .margin_top(4)
                .margin_bottom(4)
                .build();
            list_item.set_child(Some(&label));
        });

        factory.connect_bind(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let port = list_item.item().and_downcast::<PortObject>().unwrap();
            let label = list_item.child().and_downcast::<gtk::Label>().unwrap();

            label.set_text(&port.display_label());
            // Use tooltip for additional accessible description
            label.set_tooltip_text(Some(&port.accessible_description()));
        });

        // Create ListView
        let list_view = gtk::ListView::builder()
            .model(&selection)
            .factory(&factory)
            .single_click_activate(false)
            .build();

        // Store reference to list view
        if is_output {
            self.imp().output_list_view.replace(Some(list_view.clone()));
        } else {
            self.imp().input_list_view.replace(Some(list_view.clone()));
        }

        // Keyboard navigation: Enter to connect, Left/Right to switch lists, Ctrl+Down to connections
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            Propagation::Proceed,
            move |_, key, _, modifiers| {
                let ctrl = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                match key {
                    // Ctrl+Enter to connect selected ports (works from either list)
                    Key::Return | Key::KP_Enter if ctrl => {
                        window.connect_selected();
                        Propagation::Stop
                    }
                    // Ctrl+Down: jump to connections list, remember which list we came from
                    Key::Down | Key::KP_Down if ctrl => {
                        window.imp().last_port_list_was_output.replace(is_output);
                        window.focus_connections_list();
                        Propagation::Stop
                    }
                    // Right arrow: move from output to input list
                    Key::Right | Key::KP_Right if is_output => {
                        window.focus_input_list();
                        Propagation::Stop
                    }
                    // Left arrow: move from input to output list
                    Key::Left | Key::KP_Left if !is_output => {
                        window.focus_output_list();
                        Propagation::Stop
                    }
                    _ => Propagation::Proceed,
                }
            }
        ));
        list_view.add_controller(key_controller);

        // Scrolled window
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(200)
            .vexpand(true)
            .child(&list_view)
            .build();

        panel_box.append(&scrolled);

        // Connect button (only for output panel)
        if is_output {
            let connect_btn = gtk::Button::builder()
                .label("Connect")
                .tooltip_text("Connect the selected output port to the selected input port (Ctrl+Enter)")
                .build();
            connect_btn.set_action_name(Some("win.connect-selected"));
            panel_box.append(&connect_btn);
        }

        frame.set_child(Some(&panel_box));
        frame
    }

    /// Build the connections panel showing active links
    fn build_connections_panel(&self) -> gtk::Frame {
        let frame = gtk::Frame::builder()
            .label("Active Connections")
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(6)
            .build();

        // Use SingleSelection so we can select and delete with keyboard
        let selection = gtk::SingleSelection::new(Some(self.imp().links.clone()));
        self.imp().connections_selection.replace(Some(selection.clone()));

        let factory = gtk::SignalListItemFactory::new();

        factory.connect_setup(|_, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();

            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_start(6)
                .margin_end(6)
                .margin_top(4)
                .margin_bottom(4)
                .build();

            let label = gtk::Label::builder()
                .halign(gtk::Align::Start)
                .hexpand(true)
                .xalign(0.0)
                .build();

            let delete_btn = gtk::Button::builder()
                .label("Delete")
                .css_classes(["destructive-action"])
                .build();

            row.append(&label);
            row.append(&delete_btn);

            list_item.set_child(Some(&row));
        });

        factory.connect_bind(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, list_item| {
                let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
                let link = list_item.item().and_downcast::<LinkObject>().unwrap();
                let row = list_item.child().and_downcast::<gtk::Box>().unwrap();

                // Update label
                let label = row.first_child().and_downcast::<gtk::Label>().unwrap();
                label.set_text(&link.display_label());
                label.set_tooltip_text(Some(&link.accessible_description()));

                // Update delete button
                let delete_btn = row.last_child().and_downcast::<gtk::Button>().unwrap();
                delete_btn.set_tooltip_text(Some(&format!(
                    "Delete connection: {}",
                    link.display_label()
                )));

                // Connect delete action
                let link_id = link.id();
                delete_btn.connect_clicked(glib::clone!(
                    #[weak]
                    window,
                    move |_| {
                        window.delete_link(link_id);
                    }
                ));
            }
        ));

        let list_view = gtk::ListView::builder()
            .model(&selection)
            .factory(&factory)
            .build();

        // Store reference to connections list view
        self.imp().connections_list_view.replace(Some(list_view.clone()));

        // Add keyboard handler for Delete and navigation
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(glib::clone!(
            #[weak(rename_to = window)]
            self,
            #[upgrade_or]
            Propagation::Proceed,
            move |_, key, _, modifiers| {
                let ctrl = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                match key {
                    // Delete selected connection
                    Key::Delete | Key::KP_Delete | Key::BackSpace => {
                        window.delete_selected_connection();
                        Propagation::Stop
                    }
                    // Ctrl+Up: jump back to the port list we came from
                    Key::Up | Key::KP_Up if ctrl => {
                        if *window.imp().last_port_list_was_output.borrow() {
                            window.focus_output_list();
                        } else {
                            window.focus_input_list();
                        }
                        Propagation::Stop
                    }
                    _ => Propagation::Proceed,
                }
            }
        ));
        list_view.add_controller(key_controller);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(80)
            .max_content_height(150)
            .child(&list_view)
            .build();

        frame.set_child(Some(&scrolled));
        frame
    }

    /// Build the status bar
    fn build_status_bar(&self) -> gtk::Box {
        let bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(6)
            .accessible_role(gtk::AccessibleRole::Status)
            .build();

        let label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .label("Connecting to PipeWire...")
            .build();

        self.imp().status_label.replace(Some(label.clone()));
        bar.append(&label);

        bar
    }

    /// Set up window actions
    fn setup_actions(&self) {
        // Action: connect-selected
        let action_connect = gio::SimpleAction::new("connect-selected", None);
        action_connect.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.connect_selected();
            }
        ));
        self.add_action(&action_connect);

        // Action: save-preset
        let action_save = gio::SimpleAction::new("save-preset", None);
        action_save.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.show_save_preset_dialog();
            }
        ));
        self.add_action(&action_save);

        // Action: load-preset
        let action_load = gio::SimpleAction::new("load-preset", None);
        action_load.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.show_load_preset_dialog();
            }
        ));
        self.add_action(&action_load);

        // Action: deactivate-preset
        let action_deactivate = gio::SimpleAction::new("deactivate-preset", None);
        action_deactivate.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.deactivate_preset();
            }
        ));
        self.add_action(&action_deactivate);

        // Action: start-minimized (stateful toggle)
        let start_minimized = self.imp().settings.borrow().start_minimized;
        let action_start_minimized =
            gio::SimpleAction::new_stateful("start-minimized", None, &start_minimized.to_variant());
        action_start_minimized.connect_activate(glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |action, _| {
                let current = action
                    .state()
                    .and_then(|v| v.get::<bool>())
                    .unwrap_or(false);
                let new_state = !current;
                action.set_state(&new_state.to_variant());
                window.set_start_minimized(new_state);
            }
        ));
        self.add_action(&action_start_minimized);
    }

    /// Connect the selected output port to the selected input port
    fn connect_selected(&self) {
        // Get all selected output ports
        let output_ports: Vec<PortObject> = {
            let selection = self.imp().output_selection.borrow();
            match selection.as_ref() {
                Some(s) => {
                    let bitset = s.selection();
                    let mut ports = Vec::new();
                    let size = bitset.size();
                    for i in 0..size {
                        let idx = bitset.nth(i as u32);
                        if let Some(port) = s.item(idx).and_downcast::<PortObject>() {
                            ports.push(port);
                        }
                    }
                    ports
                }
                None => Vec::new(),
            }
        };

        if output_ports.is_empty() {
            self.announce("No output ports selected");
            return;
        }

        // Get all selected input ports
        let input_ports: Vec<PortObject> = {
            let selection = self.imp().input_selection.borrow();
            match selection.as_ref() {
                Some(s) => {
                    let bitset = s.selection();
                    let mut ports = Vec::new();
                    let size = bitset.size();
                    for i in 0..size {
                        let idx = bitset.nth(i as u32);
                        if let Some(port) = s.item(idx).and_downcast::<PortObject>() {
                            ports.push(port);
                        }
                    }
                    ports
                }
                None => Vec::new(),
            }
        };

        if input_ports.is_empty() {
            self.announce("No input ports selected");
            return;
        }

        // Connection modes:
        // - 1 output to N inputs: connect that output to ALL inputs (e.g., mono to stereo)
        // - N outputs to 1 input: connect ALL outputs to that input (e.g., mix down)
        // - N outputs to N inputs: connect pairwise by position (e.g., stereo to stereo)
        let mut count = 0;

        if output_ports.len() == 1 {
            // One output to multiple inputs
            let output = &output_ports[0];
            for input in &input_ports {
                self.create_link(output.id(), input.id());
                count += 1;
            }
        } else if input_ports.len() == 1 {
            // Multiple outputs to one input
            let input = &input_ports[0];
            for output in &output_ports {
                self.create_link(output.id(), input.id());
                count += 1;
            }
        } else {
            // Pairwise connection
            let pairs = output_ports.len().min(input_ports.len());
            for i in 0..pairs {
                self.create_link(output_ports[i].id(), input_ports[i].id());
                count += 1;
            }
        }

        if count > 1 {
            self.announce(&format!("Created {} connections", count));
        }
    }

    /// Create a link between two ports
    fn create_link(&self, output_port_id: u32, input_port_id: u32) {
        if let Some(tx) = self.imp().command_tx.borrow().as_ref() {
            let cmd = UiCommand::CreateLink {
                output_port_id,
                input_port_id,
            };
            if let Err(e) = tx.send_blocking(cmd) {
                log::error!("Failed to send create link command: {}", e);
            }
        }
    }

    /// Delete a link
    fn delete_link(&self, link_id: u32) {
        if let Some(tx) = self.imp().command_tx.borrow().as_ref() {
            let cmd = UiCommand::DeleteLink { link_id };
            if let Err(e) = tx.send_blocking(cmd) {
                log::error!("Failed to send delete link command: {}", e);
            }
        }
    }

    /// Delete the currently selected connection
    fn delete_selected_connection(&self) {
        let (link, selected_pos) = {
            let selection = self.imp().connections_selection.borrow();
            match selection.as_ref() {
                Some(s) => (
                    s.selected_item().and_downcast::<LinkObject>(),
                    s.selected(),
                ),
                None => (None, gtk::INVALID_LIST_POSITION),
            }
        };

        if let Some(link) = link {
            // Save position for selection restoration when LinkRemoved event arrives
            self.imp().pending_delete_position.replace(Some(selected_pos));

            // Delete the link (async - will trigger LinkRemoved event)
            self.delete_link(link.id());
        }
    }

    /// Apply current filters to the port lists
    fn apply_filters(&self) {
        let search_text = self.imp().search_text.borrow().to_lowercase();
        let show_audio = *self.imp().show_audio.borrow();
        let show_midi = *self.imp().show_midi.borrow();
        let show_video = *self.imp().show_video.borrow();

        // Create a filter function that captures the current filter state
        let filter_fn = move |obj: &glib::Object| -> bool {
            let port = match obj.downcast_ref::<PortObject>() {
                Some(p) => p,
                None => return false,
            };

            // Check media type filter
            let media_type = port.media_type();
            let media_ok = match media_type.as_str() {
                "audio" => show_audio,
                "midi" => show_midi,
                "video" => show_video,
                _ => true, // Show unknown types
            };

            if !media_ok {
                return false;
            }

            // Check search text filter
            if !search_text.is_empty() {
                let label = port.display_label().to_lowercase();
                let node_name = port.node_name().to_lowercase();
                if !label.contains(&search_text) && !node_name.contains(&search_text) {
                    return false;
                }
            }

            true
        };

        // Update output filter
        if let Some(filter) = self.imp().output_filter.borrow().as_ref() {
            filter.set_filter_func(filter_fn.clone());
        }

        // Update input filter
        if let Some(filter) = self.imp().input_filter.borrow().as_ref() {
            filter.set_filter_func(filter_fn);
        }
    }

    /// Remove a port from the lists by ID
    fn remove_port_from_lists(&self, id: u32) {
        // Remove from output ports
        for i in 0..self.imp().output_ports.n_items() {
            if let Some(port) = self.imp().output_ports.item(i).and_downcast::<PortObject>() {
                if port.id() == id {
                    self.imp().output_ports.remove(i);
                    return;
                }
            }
        }

        // Remove from input ports
        for i in 0..self.imp().input_ports.n_items() {
            if let Some(port) = self.imp().input_ports.item(i).and_downcast::<PortObject>() {
                if port.id() == id {
                    self.imp().input_ports.remove(i);
                    return;
                }
            }
        }
    }

    /// Remove a link from the list by ID
    fn remove_link_from_list(&self, id: u32) {
        let n_items = self.imp().links.n_items();
        for i in 0..n_items {
            if let Some(link) = self.imp().links.item(i).and_downcast::<LinkObject>() {
                if link.id() == id {
                    // Check if this was a user-initiated delete (pending position set)
                    let was_user_delete = self.imp().pending_delete_position.take().is_some();

                    // Remove the item
                    self.imp().links.remove(i);

                    // Restore selection and focus if this was user-initiated delete
                    if was_user_delete && n_items > 1 {
                        let new_pos = if i >= n_items - 1 {
                            // Was last item, select new last
                            i.saturating_sub(1)
                        } else {
                            // Select same position (next item slid into place)
                            i
                        };

                        // Set selection immediately
                        if let Some(selection) = self.imp().connections_selection.borrow().as_ref() {
                            selection.set_selected(new_pos);
                        }

                        // Scroll to and focus the item after GTK processes the change
                        if let Some(list_view) = self.imp().connections_list_view.borrow().clone() {
                            glib::idle_add_local_once(move || {
                                list_view.scroll_to(new_pos, gtk::ListScrollFlags::FOCUS, None);
                            });
                        }
                    }
                    return;
                }
            }
        }
    }

    /// Update the status bar
    fn update_status(&self, message: &str, _busy: bool) {
        if let Some(label) = self.imp().status_label.borrow().as_ref() {
            label.set_text(message);
        }
    }

    /// Update status with counts
    fn update_status_counts(&self) {
        let state = self.imp().pw_state.borrow();
        let msg = format!(
            "Connected | {} nodes | {} ports | {} links",
            state.nodes.len(),
            state.ports.len(),
            state.links.len()
        );
        self.update_status(&msg, false);
    }

    /// Focus the input ports list (for left/right navigation)
    fn focus_input_list(&self) {
        if let Some(list_view) = self.imp().input_list_view.borrow().as_ref() {
            list_view.grab_focus();
        }
    }

    /// Focus the output ports list (for left/right navigation)
    fn focus_output_list(&self) {
        if let Some(list_view) = self.imp().output_list_view.borrow().as_ref() {
            list_view.grab_focus();
        }
    }

    /// Focus the connections list
    fn focus_connections_list(&self) {
        if let Some(list_view) = self.imp().connections_list_view.borrow().as_ref() {
            list_view.grab_focus();
        }
    }

    /// Announce a message to screen readers
    fn announce(&self, message: &str) {
        use gtk::AccessibleAnnouncementPriority;
        self.announce_with_priority(message, AccessibleAnnouncementPriority::Medium);
    }

    /// Announce a message to screen readers with a specific priority
    fn announce_with_priority(&self, message: &str, priority: gtk::AccessibleAnnouncementPriority) {
        use gtk::prelude::AccessibleExt;
        self.upcast_ref::<gtk::Widget>().announce(message, priority);
    }

    /// Show dialog to save current connections as a preset
    fn show_save_preset_dialog(&self) {
        let dialog = adw::MessageDialog::builder()
            .transient_for(self)
            .modal(true)
            .heading("Save Preset")
            .body("Enter a name for this connection preset:")
            .build();

        // Add entry for preset name
        let entry = gtk::Entry::builder()
            .placeholder_text("Preset name")
            .activates_default(true)
            .build();
        dialog.set_extra_child(Some(&entry));

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("save", "Save");
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        dialog.connect_response(
            None,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[weak]
                entry,
                move |dialog, response| {
                    dialog.close();
                    if response == "save" {
                        let name = entry.text().trim().to_string();
                        if name.is_empty() {
                            window.announce("Preset name cannot be empty");
                            return;
                        }
                        window.save_preset(&name);
                    }
                }
            ),
        );

        dialog.present();
        entry.grab_focus();
    }

    /// Save current connections as a preset
    fn save_preset(&self, name: &str) {
        let connections: Vec<PresetConnection> = {
            let pw_state = self.imp().pw_state.borrow();
            pw_state
                .links
                .values()
                .filter_map(|link| {
                    let output_port = pw_state.ports.get(&link.output_port_id)?;
                    let input_port = pw_state.ports.get(&link.input_port_id)?;
                    let output_node = pw_state.nodes.get(&output_port.node_id)?;
                    let input_node = pw_state.nodes.get(&input_port.node_id)?;

                    Some(PresetConnection {
                        output_node: output_node.name.clone(),
                        output_port: output_port.name.clone(),
                        input_node: input_node.name.clone(),
                        input_port: input_port.name.clone(),
                    })
                })
                .collect()
        };

        if connections.is_empty() {
            self.announce("No connections to save");
            return;
        }

        let preset = Preset {
            name: name.to_string(),
            connections,
        };

        let count = preset.connections.len();
        self.imp().preset_store.borrow_mut().add_preset(preset);

        if let Err(e) = self.imp().preset_store.borrow().save() {
            self.announce(&format!("Failed to save preset: {}", e));
        } else {
            self.announce(&format!("Saved preset \"{}\" with {} connections", name, count));
        }
    }

    /// Show dialog to load a preset
    fn show_load_preset_dialog(&self) {
        let preset_names = self.imp().preset_store.borrow().preset_names();
        let active_preset = self.imp().preset_store.borrow().active_preset.clone();

        if preset_names.is_empty() {
            self.announce("No presets saved yet");
            return;
        }

        let dialog = adw::MessageDialog::builder()
            .transient_for(self)
            .modal(true)
            .heading("Manage Presets")
            .body("Select a preset. Use 'Activate' for auto-connect or 'Load' for one-time.")
            .build();

        // Create a list box with preset options
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();

        for name in &preset_names {
            let is_active = active_preset.as_deref() == Some(name.as_str());
            let row = adw::ActionRow::builder()
                .title(name)
                .subtitle(if is_active { "Active (auto-connecting)" } else { "" })
                .activatable(true)
                .build();

            // Add a checkmark icon for active preset
            if is_active {
                let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
                icon.set_tooltip_text(Some("Currently active"));
                row.add_suffix(&icon);
            }

            list_box.append(&row);
        }

        // Select first item
        if let Some(first_row) = list_box.row_at_index(0) {
            list_box.select_row(Some(&first_row));
        }

        // Wrap in scrolled window for long lists
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(100)
            .max_content_height(300)
            .child(&list_box)
            .build();

        dialog.set_extra_child(Some(&scrolled));

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.add_response("load", "Load Once");
        dialog.add_response("activate", "Activate");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("activate", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("activate"));
        dialog.set_close_response("cancel");

        // Handle row activation (double-click or Enter)
        let dialog_weak = dialog.downgrade();
        list_box.connect_row_activated(move |_, _| {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.response("activate");
            }
        });

        dialog.connect_response(
            None,
            glib::clone!(
                #[weak(rename_to = window)]
                self,
                #[weak]
                list_box,
                move |dialog, response| {
                    let selected_name = list_box.selected_row().and_then(|row| {
                        row.downcast::<adw::ActionRow>()
                            .ok()
                            .map(|ar| ar.title().to_string())
                    });

                    match response {
                        "activate" => {
                            dialog.close();
                            if let Some(name) = selected_name {
                                window.activate_preset(&name);
                            }
                        }
                        "load" => {
                            dialog.close();
                            if let Some(name) = selected_name {
                                window.load_preset(&name);
                            }
                        }
                        "delete" => {
                            if let Some(name) = selected_name.clone() {
                                window.delete_preset(&name);
                                // Refresh dialog or close if no presets left
                                let remaining = window.imp().preset_store.borrow().preset_names();
                                if remaining.is_empty() {
                                    dialog.close();
                                    window.announce("No presets remaining");
                                } else {
                                    // Remove the row from list
                                    if let Some(row) = list_box.selected_row() {
                                        list_box.remove(&row);
                                        // Select first remaining
                                        if let Some(first) = list_box.row_at_index(0) {
                                            list_box.select_row(Some(&first));
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            dialog.close();
                        }
                    }
                }
            ),
        );

        dialog.present();
        list_box.grab_focus();
    }

    /// Load a preset by name
    fn load_preset(&self, name: &str) {
        let preset = {
            let store = self.imp().preset_store.borrow();
            store.get_preset(name).cloned()
        };

        let preset = match preset {
            Some(p) => p,
            None => {
                self.announce(&format!("Preset \"{}\" not found", name));
                return;
            }
        };

        // Collect links to create (to avoid borrow issues)
        let links_to_create: Vec<(u32, u32)>;
        let mut skipped = 0;

        {
            let pw_state = self.imp().pw_state.borrow();
            let mut to_create = Vec::new();

            for conn in &preset.connections {
                // Find output port by node name and port name
                let output_port = pw_state.ports.values().find(|p| {
                    p.direction == PortDirection::Output
                        && p.name == conn.output_port
                        && pw_state
                            .nodes
                            .get(&p.node_id)
                            .map(|n| n.name == conn.output_node)
                            .unwrap_or(false)
                });

                // Find input port by node name and port name
                let input_port = pw_state.ports.values().find(|p| {
                    p.direction == PortDirection::Input
                        && p.name == conn.input_port
                        && pw_state
                            .nodes
                            .get(&p.node_id)
                            .map(|n| n.name == conn.input_node)
                            .unwrap_or(false)
                });

                match (output_port, input_port) {
                    (Some(out), Some(inp)) => {
                        // Check if link already exists
                        let exists = pw_state.links.values().any(|l| {
                            l.output_port_id == out.id && l.input_port_id == inp.id
                        });

                        if !exists {
                            to_create.push((out.id, inp.id));
                        } else {
                            skipped += 1;
                        }
                    }
                    _ => {
                        skipped += 1;
                        log::debug!(
                            "Could not find ports for connection: {} -> {}",
                            conn.output_port,
                            conn.input_port
                        );
                    }
                }
            }

            links_to_create = to_create;
        }

        // Now create the links (pw_state borrow is released)
        let created = links_to_create.len();
        for (output_id, input_id) in links_to_create {
            self.create_link(output_id, input_id);
        }

        if created > 0 && skipped == 0 {
            self.announce(&format!("Loaded preset \"{}\": {} connections", name, created));
        } else if created > 0 {
            self.announce(&format!(
                "Loaded preset \"{}\": {} created, {} skipped",
                name, created, skipped
            ));
        } else if skipped > 0 {
            self.announce(&format!(
                "Preset \"{}\": all {} connections already exist or unavailable",
                name, skipped
            ));
        }
    }

    /// Delete a preset by name
    fn delete_preset(&self, name: &str) {
        // If deleting the active preset, deactivate it first
        let was_active = self.imp().preset_store.borrow().is_active(name);
        if was_active {
            self.imp().preset_store.borrow_mut().deactivate_preset();
        }

        self.imp().preset_store.borrow_mut().remove_preset(name);

        if let Err(e) = self.imp().preset_store.borrow().save() {
            self.announce(&format!("Failed to save after delete: {}", e));
        } else {
            self.announce(&format!("Deleted preset \"{}\"", name));
        }

        // Update display if we deactivated the preset
        if was_active {
            self.update_active_preset_display();
        }
    }

    /// Check and create auto-connections for the active preset
    /// Called when a new port is added to see if it completes any preset connections
    fn check_auto_connect(&self) {
        // Get the active preset's connections
        let preset_connections: Vec<PresetConnection> = {
            let store = self.imp().preset_store.borrow();
            match store.get_active_preset() {
                Some(preset) => preset.connections.clone(),
                None => return, // No active preset
            }
        };

        // Check each connection in the preset
        let pw_state = self.imp().pw_state.borrow();
        let mut links_to_create = Vec::new();

        for conn in &preset_connections {
            // Find output port by node name and port name
            let output_port = pw_state.ports.values().find(|p| {
                p.direction == PortDirection::Output
                    && p.name == conn.output_port
                    && pw_state
                        .nodes
                        .get(&p.node_id)
                        .map(|n| n.name == conn.output_node)
                        .unwrap_or(false)
            });

            // Find input port by node name and port name
            let input_port = pw_state.ports.values().find(|p| {
                p.direction == PortDirection::Input
                    && p.name == conn.input_port
                    && pw_state
                        .nodes
                        .get(&p.node_id)
                        .map(|n| n.name == conn.input_node)
                        .unwrap_or(false)
            });

            // If both ports exist and link doesn't already exist, queue it
            if let (Some(out), Some(inp)) = (output_port, input_port) {
                let link_key = (out.id, inp.id);

                // Check if link already exists
                let exists = pw_state
                    .links
                    .values()
                    .any(|l| l.output_port_id == out.id && l.input_port_id == inp.id);

                // Check if link creation is already in-flight
                let pending = self.imp().pending_links.borrow().contains(&link_key);

                if !exists && !pending {
                    links_to_create.push(link_key);
                }
            }
        }

        // Release borrow before creating links
        drop(pw_state);

        // Mark links as pending and create them
        {
            let mut pending = self.imp().pending_links.borrow_mut();
            for &link_key in &links_to_create {
                pending.insert(link_key);
            }
        }

        // Create the links
        let count = links_to_create.len();
        for (output_id, input_id) in links_to_create {
            log::debug!("Auto-connecting ports {} -> {}", output_id, input_id);
            self.create_link(output_id, input_id);
        }

        // Notify user of auto-connections (for accessibility)
        if count > 0 {
            if count == 1 {
                self.announce("Auto-connected 1 port");
            } else {
                self.announce(&format!("Auto-connected {} ports", count));
            }
        }
    }

    /// Activate a preset for auto-connecting
    pub fn activate_preset(&self, name: &str) {
        {
            let mut store = self.imp().preset_store.borrow_mut();
            store.activate_preset(name);
        }

        // Save the activation state
        if let Err(e) = self.imp().preset_store.borrow().save() {
            self.announce(&format!("Failed to save: {}", e));
            return;
        }

        // Immediately try to establish any connections
        self.check_auto_connect();

        self.announce(&format!("Activated preset \"{}\"", name));
        self.update_active_preset_display();
    }

    /// Deactivate the current preset
    pub fn deactivate_preset(&self) {
        let name = {
            let store = self.imp().preset_store.borrow();
            store.active_preset.clone()
        };

        // Nothing to deactivate
        if name.is_none() {
            self.announce("No preset is currently active");
            return;
        }

        {
            self.imp().preset_store.borrow_mut().deactivate_preset();
        }

        if let Err(e) = self.imp().preset_store.borrow().save() {
            self.announce(&format!("Failed to save: {}", e));
            return;
        }

        if let Some(name) = name {
            self.announce(&format!("Deactivated preset \"{}\"", name));
        }
        self.update_active_preset_display();
    }

    /// Update the UI to show which preset is active
    fn update_active_preset_display(&self) {
        let active_name = {
            let store = self.imp().preset_store.borrow();
            store.active_preset.clone()
        };

        // Update subtitle to show active preset
        if let Some(name) = active_name {
            self.set_title(Some(&format!("PW Audioshare - [{}]", name)));
        } else {
            self.set_title(Some("PW Audioshare"));
        }
    }

    /// Set the start minimized setting and save it
    fn set_start_minimized(&self, minimized: bool) {
        {
            let mut settings = self.imp().settings.borrow_mut();
            settings.start_minimized = minimized;
        }

        if let Err(e) = self.imp().settings.borrow().save() {
            self.announce(&format!("Failed to save settings: {}", e));
            return;
        }

        if minimized {
            self.announce("Will start minimized to tray");
        } else {
            self.announce("Will start with window visible");
        }
    }
}
