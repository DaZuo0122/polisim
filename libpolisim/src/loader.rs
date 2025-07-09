use std::collections::HashMap;
use std::fs;
use std::path::Path;

use nalgebra::DVector;
use petgraph::graph::NodeIndex;
use serde::Deserialize;

use crate::sim::{CongressGraph, Node, Party};

/// Top‐level TOML structure with members, parties, and edges.
#[derive(Deserialize)]
struct RawConfig {
    ideal_dimension: usize,
    congress_members: Vec<RawMember>,
    parties: Vec<RawParty>,
    edges: Option<Vec<RawEdge>>,
}

#[derive(Deserialize)]
struct RawMember {
    id: String,
    ideal: Vec<f64>,
    bias: f64,
    swing: f64,
}

#[derive(Deserialize)]
struct RawParty {
    id: String,
    discipline: f64,
    members: Vec<String>,
}

#[derive(Deserialize)]
struct RawEdge {
    from: String,
    to: String,
    weight: f64,
}

/// Load and build a `CongressGraph` from a TOML file.
pub fn load_congress_graph_from_toml<P: AsRef<Path>>(
    path: P,
) -> Result<CongressGraph, Box<dyn std::error::Error>> {
    // 1) Read & parse the TOML
    let toml_str = fs::read_to_string(path)?;
    let raw: RawConfig = toml::from_str(&toml_str)?;

    // 2) Create an empty CongressGraph
    let mut cg = CongressGraph::new();

    // 3) Insert all nodes, checking dimension
    let mut index_map: HashMap<String, NodeIndex> = HashMap::new();
    for rm in raw.congress_members {
        if rm.ideal.len() != raw.ideal_dimension {
            return Err(format!(
                "Member `{}` has ideal length {}, but ideal_dimension = {}",
                rm.id,
                rm.ideal.len(),
                raw.ideal_dimension
            )
            .into());
        }

        let node = Node {
            id: rm.id.clone(),
            ideal: DVector::from_vec(rm.ideal),
            bias: rm.bias,
            swing: rm.swing,
        };
        let idx = cg.add_node(node);
        index_map.insert(rm.id, idx);
    }

    // 4) Insert edges if any
    if let Some(edges) = raw.edges {
        for e in edges {
            let from_idx = index_map
                .get(&e.from)
                .ok_or_else(|| format!("Unknown edge.from node `{}`", e.from))?;
            let to_idx = index_map
                .get(&e.to)
                .ok_or_else(|| format!("Unknown edge.to node `{}`", e.to))?;
            cg.add_edge(*from_idx, *to_idx, e.weight);
        }
    }

    // 5) Insert parties
    for rp in raw.parties {
        let mut members_idx = Vec::with_capacity(rp.members.len());
        for mem_id in rp.members {
            let &ni = index_map.get(&mem_id).ok_or_else(|| {
                format!("Party `{}` refers to unknown member `{}`", rp.id, mem_id)
            })?;
            members_idx.push(ni);
        }
        let party = Party {
            id: rp.id,
            discipline: rp.discipline,
            members: members_idx,
        };
        cg.add_party(party);
    }

    Ok(cg)
}
