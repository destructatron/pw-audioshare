use std::collections::HashMap;

use super::messages::{LinkState, MediaType, PortDirection};

/// Represents a PipeWire node (audio device, application, etc.)
#[derive(Debug, Clone)]
pub struct PwNode {
    pub id: u32,
    pub name: String,
    pub media_class: Option<String>,
    pub description: Option<String>,
    pub application_name: Option<String>,
}

impl PwNode {
    /// Returns the best display name for this node
    pub fn display_name(&self) -> &str {
        self.description
            .as_deref()
            .or(self.application_name.as_deref())
            .unwrap_or(&self.name)
    }
}

/// Represents a port on a node
#[derive(Debug, Clone)]
pub struct PwPort {
    pub id: u32,
    pub node_id: u32,
    pub name: String,
    pub alias: Option<String>,
    pub direction: PortDirection,
    pub media_type: MediaType,
    pub channel: Option<String>,
}

impl PwPort {
    /// Returns the best display name for this port
    pub fn display_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// Represents a link between two ports
#[derive(Debug, Clone)]
pub struct PwLink {
    pub id: u32,
    pub output_node_id: u32,
    pub output_port_id: u32,
    pub input_node_id: u32,
    pub input_port_id: u32,
    pub state: LinkState,
}

/// Holds the complete PipeWire state as seen by the application
#[derive(Debug, Default)]
pub struct PwState {
    pub nodes: HashMap<u32, PwNode>,
    pub ports: HashMap<u32, PwPort>,
    pub links: HashMap<u32, PwLink>,
}

impl PwState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the node that owns a port
    pub fn get_port_node(&self, port_id: u32) -> Option<&PwNode> {
        self.ports
            .get(&port_id)
            .and_then(|port| self.nodes.get(&port.node_id))
    }

    /// Get all ports for a node
    pub fn get_node_ports(&self, node_id: u32) -> impl Iterator<Item = &PwPort> {
        self.ports.values().filter(move |p| p.node_id == node_id)
    }

    /// Get all output ports (sources)
    pub fn output_ports(&self) -> impl Iterator<Item = &PwPort> {
        self.ports
            .values()
            .filter(|p| p.direction == PortDirection::Output)
    }

    /// Get all input ports (sinks)
    pub fn input_ports(&self) -> impl Iterator<Item = &PwPort> {
        self.ports
            .values()
            .filter(|p| p.direction == PortDirection::Input)
    }

    /// Check if a link exists between two ports
    pub fn link_exists(&self, output_port_id: u32, input_port_id: u32) -> bool {
        self.links.values().any(|link| {
            link.output_port_id == output_port_id && link.input_port_id == input_port_id
        })
    }

    /// Find link by port IDs
    pub fn find_link(&self, output_port_id: u32, input_port_id: u32) -> Option<&PwLink> {
        self.links.values().find(|link| {
            link.output_port_id == output_port_id && link.input_port_id == input_port_id
        })
    }
}
