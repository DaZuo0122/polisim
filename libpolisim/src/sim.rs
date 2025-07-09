use nalgebra::DVector;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use rand::seq::SliceRandom;
use rand::{Rng, thread_rng};
use std::collections::HashMap;

// Node attributes representing a congress member
pub struct Node {
    pub id: String,
    pub ideal: DVector<f64>,
    pub bias: f64,
    pub swing: f64,
}

// Party structure with members and discipline factor
pub struct Party {
    pub id: String,
    pub discipline: f64,
    pub members: Vec<NodeIndex>,
}

// Main simulation graph structure
pub struct CongressGraph {
    pub graph: DiGraph<Node, f64>,
    parties: Vec<Party>,
    node_party_map: HashMap<NodeIndex, usize>,
}

/// Common types of passing threshold
pub enum Majority {
    /// yes > 50%, abstentions do not count
    SIMPLE,
    /// yes > 2/3, abstentions do not count
    SUPER,
    /// yes > 50%, abstentions count against(as no)
    ABSSIMPLE,
    /// yes > 2/3, abstentions count against(as no)
    ABSSUPER,
    /// 100% yes required(abstention will block)
    UNANIMITY,
}

impl CongressGraph {
    /// Creates a new empty CongressGraph
    pub fn new() -> Self {
        CongressGraph {
            graph: DiGraph::new(),
            parties: Vec::new(),
            node_party_map: HashMap::new(),
        }
    }

    /// Adds a new congress member node to the graph
    pub fn add_node(&mut self, node: Node) -> NodeIndex {
        self.graph.add_node(node)
    }

    /// Adds an influence edge between two nodes
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, weight: f64) {
        self.graph.add_edge(from, to, weight);
    }

    /// Adds a party to the graph
    pub fn add_party(&mut self, party: Party) {
        let party_idx = self.parties.len();
        for &member in &party.members {
            self.node_party_map.insert(member, party_idx);
        }
        self.parties.push(party);
    }

    /// Retrieves party index for a node
    pub fn get_party_index(&self, node_idx: NodeIndex) -> Option<usize> {
        self.node_party_map.get(&node_idx).copied()
    }

    /// Gets party reference by index
    pub fn get_party(&self, party_idx: usize) -> Option<&Party> {
        self.parties.get(party_idx)
    }
}

// Simulator for running voting simulations
pub struct Simulator<'a> {
    congress: &'a CongressGraph,
    proposal: DVector<f64>,
    scores: Vec<f64>,
    votes: Vec<i8>,
}

impl<'a> Simulator<'a> {
    /// Creates a new simulator for a given proposal
    pub fn new(congress: &'a CongressGraph, proposal: DVector<f64>) -> Self {
        let node_count = congress.graph.node_count();
        let mut scores = vec![0.0; node_count];

        // Initialize scores based on policy alignment + personal bias
        for node_idx in congress.graph.node_indices() {
            let node = &congress.graph[node_idx];
            let alignment = cosine_similarity(&node.ideal, &proposal);
            scores[node_idx.index()] = alignment + node.bias;
        }

        Simulator {
            congress,
            proposal,
            scores,
            votes: vec![0; node_count],
        }
    }

    /// Runs the simulation for specified number of rounds
    pub fn run(&mut self, max_rounds: usize, threshold: f64) {
        let mut rng = thread_rng();
        let node_indices: Vec<NodeIndex> = self.congress.graph.node_indices().collect();

        for _ in 0..max_rounds {
            let mut order = node_indices.clone();
            order.shuffle(&mut rng);

            for &node_idx in &order {
                // Calculate peer pressure from influences
                let peer_pressure = self.calculate_peer_pressure(node_idx);

                // Calculate party discipline pressure
                let party_pressure = self.calculate_party_pressure(node_idx);

                // Update node score
                self.update_node_score(node_idx, peer_pressure + party_pressure);
            }
        }

        // Finalize votes using threshold
        for node_idx in self.congress.graph.node_indices() {
            let score = self.scores[node_idx.index()];
            self.votes[node_idx.index()] = if score > threshold {
                1
            } else if score < -threshold {
                -1
            } else {
                0
            };
        }
    }

    /// Calculate peer pressure from incoming influences
    fn calculate_peer_pressure(&self, node_idx: NodeIndex) -> f64 {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for edge in self
            .congress
            .graph
            .edges_directed(node_idx, petgraph::Direction::Incoming)
        {
            let source_idx = edge.source();
            let weight = *edge.weight();
            let source_score = self.scores[source_idx.index()].signum();

            weighted_sum += weight * source_score;
            total_weight += weight;
        }

        if total_weight.abs() > f64::EPSILON {
            weighted_sum / total_weight
        } else {
            0.0
        }
    }

    /// Calculate party discipline pressure
    fn calculate_party_pressure(&self, node_idx: NodeIndex) -> f64 {
        self.congress
            .get_party_index(node_idx)
            .and_then(|party_idx| self.congress.get_party(party_idx))
            .map(|party| {
                let mut total_vote = 0.0;
                let mut count = 0;

                for &member in &party.members {
                    total_vote += self.scores[member.index()].signum();
                    count += 1;
                }

                // Avoid division by zero for empty parties
                if count == 0 {
                    0.0
                } else {
                    party.discipline * (total_vote / count as f64)
                }
            })
            .unwrap_or(0.0) // No party affiliation
    }

    /// Update node score based on social pressure
    fn update_node_score(&mut self, node_idx: NodeIndex, social_pressure: f64) {
        let node = &self.congress.graph[node_idx];
        let swing_factor = node.swing;
        let current_score = self.scores[node_idx.index()];

        self.scores[node_idx.index()] =
            (1.0 - swing_factor) * current_score + swing_factor * social_pressure;
    }

    /// Get final votes of all nodes,
    /// return a HashMap with node ID as key
    pub fn get_votes(&self) -> std::collections::HashMap<String, i8> {
        let mut map = std::collections::HashMap::new();
        for node_idx in self.congress.graph.node_indices() {
            let node = &self.congress.graph[node_idx];
            let vote = self.votes[node_idx.index()];
            map.insert(node.id.clone(), vote);
        }
        map
    }

    /// Get the vote result(proposal passes or not)
    pub fn passes(&self, rule: Majority) -> bool {
        // Count votes
        let mut yes = 0usize;
        let mut no = 0usize;
        let mut abstain = 0usize;

        for &v in &self.votes {
            match v {
                1 => yes += 1,
                -1 => no += 1,
                0 => abstain += 1,
                _ => unreachable!("votes should only be -1, 0, or 1"),
            }
        }

        let total_cast = yes + no; // excludes abstentions
        let total_all = yes + no + abstain;

        match rule {
            Majority::SIMPLE => {
                // yes / (yes+no) > 0.5
                if total_cast == 0 {
                    false
                } else {
                    (yes as f64) / (total_cast as f64) > 0.5
                }
            }
            Majority::SUPER => {
                // yes / (yes+no) > 2/3
                if total_cast == 0 {
                    false
                } else {
                    (yes as f64) / (total_cast as f64) > (2.0 / 3.0)
                }
            }
            Majority::ABSSIMPLE => {
                // yes / total_all > 0.5
                if total_all == 0 {
                    false
                } else {
                    (yes as f64) / (total_all as f64) > 0.5
                }
            }
            Majority::ABSSUPER => {
                // yes / total_all > 2/3
                if total_all == 0 {
                    false
                } else {
                    (yes as f64) / (total_all as f64) > (2.0 / 3.0)
                }
            }
            Majority::UNANIMITY => {
                // yes == total_all
                total_all > 0 && yes == total_all
            }
        }
    }

    /// Get final vote of a node
    pub fn get_vote(&self, node_idx: NodeIndex) -> i8 {
        self.votes[node_idx.index()]
    }

    /// Get current score of a node
    pub fn get_score(&self, node_idx: NodeIndex) -> f64 {
        self.scores[node_idx.index()]
    }
}

/// Computes cosine similarity between two vectors
pub fn cosine_similarity(a: &DVector<f64>, b: &DVector<f64>) -> f64 {
    let dot_product = a.dot(b);
    let norm_a = a.norm();
    let norm_b = b.norm();

    if norm_a.abs() < f64::EPSILON || norm_b.abs() < f64::EPSILON {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Generate dummy proposal vector, should only be used for test propose
/// Recevice a dimension and a positive f64 as upper range.
pub fn gen_random_proposal(ideal_dimension: usize, upper_range: f64) -> DVector<f64> {
    let mut rng = rand::thread_rng();
    let data: Vec<f64> = (0..ideal_dimension)
        .map(|_| rng.gen_range(-upper_range..upper_range))
        .collect();
    DVector::from_vec(data)
}

/*
example usage(for test only, better load config from toml file)
use polisimlib::sim::*;

let mut congress = CongressGraph::new();

// Add nodes
let a1 = congress.add_node(Node {
    id: "A1".into(),
    ideal: DVector::from_vec(vec![1.0, -0.5, 0.0]),
    bias: 0.2,
    swing: 0.7,
});
// Add other nodes...

// Add edges
congress.add_edge(a1, a2, 0.5);
// Add other edges...

// Add parties
congress.add_party(Party {
    id: "Party A".into(),
    discipline: 0.8,
    members: vec![a1, a2, a3],
});
// Add other parties...

// Run simulation
let proposal = DVector::from_vec(vec![0.9, -0.2, 0.1]);
let mut simulator = Simulator::new(&congress, proposal);
simulator.run(5, 0.1); // 5 rounds, ±0.1 threshold

// Get results
println!("A1 vote: {:?}", simulator.get_vote(a1));

*/
