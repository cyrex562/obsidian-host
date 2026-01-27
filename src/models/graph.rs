use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,    // Unique identifier (usually file path)
    pub label: String, // Display name (filename)
    pub node_type: NodeType,
    pub size: f32, // Visual size of the node (can depend on backlink count)
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    File,
    Image,
    Tag,
    Attachment,
    Virtual, // For non-existing files that are linked to
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String, // ID of source node
    pub target: String, // ID of target node
    pub count: u32,     // Number of links (strength of edge)
    pub edge_type: EdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Link,  // [[link]]
    Embed, // ![[embed]]
    Tag,   // File -> Tag relationship
}
