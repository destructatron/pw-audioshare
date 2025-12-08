/// Direction of a port (input receives data, output sends data)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortDirection {
    Input,
    Output,
}

impl PortDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            PortDirection::Input => "input",
            PortDirection::Output => "output",
        }
    }
}

/// Type of media carried by a port
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MediaType {
    #[default]
    Audio,
    Midi,
    Video,
    Unknown,
}

impl MediaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MediaType::Audio => "audio",
            MediaType::Midi => "midi",
            MediaType::Video => "video",
            MediaType::Unknown => "unknown",
        }
    }

    pub fn from_format_dsp(format: Option<&str>) -> Self {
        match format {
            Some(s) if s.contains("midi") => MediaType::Midi,
            Some(s) if s.contains("video") => MediaType::Video,
            Some(s) if s.contains("audio") || s.contains("32 bit float") => MediaType::Audio,
            _ => MediaType::Unknown,
        }
    }
}

/// State of a link between ports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LinkState {
    #[default]
    Active,
    Paused,
    Error,
}

impl LinkState {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkState::Active => "active",
            LinkState::Paused => "paused",
            LinkState::Error => "error",
        }
    }
}

/// Events sent from the PipeWire thread to the UI thread
#[derive(Debug, Clone)]
pub enum PwEvent {
    /// A new node appeared in the registry
    NodeAdded {
        id: u32,
        name: String,
        media_class: Option<String>,
        description: Option<String>,
        application_name: Option<String>,
    },

    /// A node was removed from the registry
    NodeRemoved { id: u32 },

    /// A new port appeared in the registry
    PortAdded {
        id: u32,
        node_id: u32,
        name: String,
        alias: Option<String>,
        direction: PortDirection,
        media_type: MediaType,
        channel: Option<String>,
    },

    /// A port was removed from the registry
    PortRemoved { id: u32 },

    /// A new link was created between ports
    LinkAdded {
        id: u32,
        output_node_id: u32,
        output_port_id: u32,
        input_node_id: u32,
        input_port_id: u32,
        state: LinkState,
    },

    /// A link was removed
    LinkRemoved { id: u32 },

    /// The state of a link changed
    LinkStateChanged { id: u32, state: LinkState },

    /// PipeWire connection established
    Connected,

    /// PipeWire connection lost or failed
    Disconnected { reason: String },

    /// An error occurred
    Error { message: String },
}

/// Commands sent from the UI thread to the PipeWire thread
#[derive(Debug, Clone)]
pub enum UiCommand {
    /// Create a link between two ports
    CreateLink {
        output_port_id: u32,
        input_port_id: u32,
    },

    /// Delete an existing link
    DeleteLink { link_id: u32 },

    /// Shutdown the PipeWire thread
    Quit,
}
