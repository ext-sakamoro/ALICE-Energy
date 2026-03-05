//! Power grid model.

use crate::node::{NodeId, NodeKind, PowerNode};
use std::collections::HashMap;

/// Unique grid identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridId(pub u64);

/// Transmission line between two nodes.
#[derive(Debug, Clone)]
pub struct Transmission {
    pub from: NodeId,
    pub to: NodeId,
    pub impedance_ohm: f64,
    pub max_capacity_mw: f64,
    pub current_flow_mw: f64,
    pub loss_fraction: f64,
}

/// Power grid containing nodes and transmission lines.
#[derive(Debug, Clone)]
pub struct PowerGrid {
    pub id: GridId,
    pub nodes: Vec<PowerNode>,
    pub transmissions: Vec<Transmission>,
    pub nominal_frequency_hz: f64,
    pub timestamp_ns: u64,
    /// O(1) node lookup: NodeId.0 → index in `nodes`.
    node_index: HashMap<u64, usize>,
    /// Adjacency list: NodeId.0 → list of connected NodeId.0 values.
    adjacency: HashMap<u64, Vec<u64>>,
}

impl PowerGrid {
    #[must_use] 
    pub fn new(id: u64, nominal_freq: f64) -> Self {
        Self {
            id: GridId(id),
            nodes: Vec::new(),
            transmissions: Vec::new(),
            nominal_frequency_hz: nominal_freq,
            timestamp_ns: 0,
            node_index: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: PowerNode) -> usize {
        let idx = self.nodes.len();
        self.node_index.insert(node.id.0, idx);
        self.nodes.push(node);
        idx
    }

    pub fn add_transmission(
        &mut self,
        from: u64,
        to: u64,
        impedance: f64,
        max_capacity: f64,
    ) -> usize {
        self.adjacency.entry(from).or_default().push(to);
        self.adjacency.entry(to).or_default().push(from);
        self.transmissions.push(Transmission {
            from: NodeId(from),
            to: NodeId(to),
            impedance_ohm: impedance,
            max_capacity_mw: max_capacity,
            current_flow_mw: 0.0,
            loss_fraction: 0.02, // default 2% loss
        });
        self.transmissions.len() - 1
    }

    /// Total generation (sum of Generator outputs).
    #[must_use] 
    pub fn total_generation(&self) -> f64 {
        self.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Generator)
            .map(|n| n.current_output_mw)
            .sum()
    }

    /// Total consumption (sum of Consumer outputs).
    #[must_use] 
    pub fn total_consumption(&self) -> f64 {
        self.nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Consumer)
            .map(|n| n.current_output_mw)
            .sum()
    }

    /// Supply minus demand (positive = surplus).
    #[inline]
    #[must_use] 
    pub fn supply_demand_balance(&self) -> f64 {
        self.total_generation() - self.total_consumption()
    }

    /// Average frequency deviation from nominal.
    #[must_use] 
    pub fn frequency_deviation(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let rcp_n = 1.0 / self.nodes.len() as f64;
        let avg: f64 = self.nodes.iter().map(|n| n.frequency_hz).sum::<f64>() * rcp_n;
        avg - self.nominal_frequency_hz
    }

    #[must_use] 
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use] 
    pub fn find_node(&self, id: u64) -> Option<&PowerNode> {
        self.nodes.iter().find(|n| n.id == NodeId(id))
    }

    pub fn find_node_mut(&mut self, id: u64) -> Option<&mut PowerNode> {
        self.nodes.iter_mut().find(|n| n.id == NodeId(id))
    }

    /// O(1) node lookup via `HashMap`.
    #[must_use] 
    pub fn find_node_fast(&self, id: u64) -> Option<&PowerNode> {
        self.node_index.get(&id).map(|&idx| &self.nodes[idx])
    }

    /// Get neighbor node IDs for a given node.
    #[must_use] 
    pub fn neighbors(&self, id: u64) -> &[u64] {
        self.adjacency.get(&id).map_or(&[], std::vec::Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;

    #[test]
    fn grid_balance_surplus() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));
        assert!((grid.supply_demand_balance() - 40.0).abs() < 1e-10);
    }

    #[test]
    fn grid_balance_deficit() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 50.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 80.0, 50.0, 220.0));
        assert!(grid.supply_demand_balance() < 0.0);
    }

    #[test]
    fn frequency_deviation_zero() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        assert!((grid.frequency_deviation()).abs() < 1e-10);
    }

    #[test]
    fn find_node() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(42, NodeKind::Generator, 100.0, 50.0, 220.0));
        assert!(grid.find_node(42).is_some());
        assert!(grid.find_node(99).is_none());
    }

    #[test]
    fn add_transmission() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));
        let idx = grid.add_transmission(1, 2, 0.5, 200.0);
        assert_eq!(idx, 0);
        assert_eq!(grid.transmissions.len(), 1);
    }

    #[test]
    fn node_count() {
        let mut grid = PowerGrid::new(1, 50.0);
        assert_eq!(grid.node_count(), 0);
        grid.add_node(PowerNode::new(1, NodeKind::Relay, 0.0, 50.0, 110.0));
        assert_eq!(grid.node_count(), 1);
    }

    #[test]
    fn find_node_fast_lookup() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(42, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(99, NodeKind::Consumer, 60.0, 50.0, 220.0));
        assert!(grid.find_node_fast(42).is_some());
        assert!(grid.find_node_fast(99).is_some());
        assert!(grid.find_node_fast(1).is_none());
    }

    #[test]
    fn adjacency_neighbors() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(3, NodeKind::Consumer, 40.0, 50.0, 220.0));
        grid.add_transmission(1, 2, 0.5, 200.0);
        grid.add_transmission(1, 3, 0.3, 150.0);
        let n1 = grid.neighbors(1);
        assert_eq!(n1.len(), 2);
        assert!(n1.contains(&2));
        assert!(n1.contains(&3));
        let n2 = grid.neighbors(2);
        assert_eq!(n2.len(), 1);
        assert!(n2.contains(&1));
        assert!(grid.neighbors(99).is_empty());
    }

    #[test]
    fn empty_grid_balance_is_zero() {
        let grid = PowerGrid::new(1, 50.0);
        assert_eq!(grid.supply_demand_balance(), 0.0);
        assert_eq!(grid.total_generation(), 0.0);
        assert_eq!(grid.total_consumption(), 0.0);
    }

    #[test]
    fn frequency_deviation_empty_grid() {
        let grid = PowerGrid::new(1, 50.0);
        assert_eq!(grid.frequency_deviation(), 0.0);
    }

    #[test]
    fn frequency_deviation_with_offset_nodes() {
        let mut grid = PowerGrid::new(1, 50.0);
        let mut n1 = PowerNode::new(1, NodeKind::Generator, 100.0, 50.1, 220.0);
        n1.frequency_hz = 50.1;
        let mut n2 = PowerNode::new(2, NodeKind::Generator, 100.0, 49.9, 220.0);
        n2.frequency_hz = 49.9;
        grid.add_node(n1);
        grid.add_node(n2);
        // Average frequency = 50.0, nominal = 50.0, deviation = 0.0
        assert!(grid.frequency_deviation().abs() < 1e-10);
    }

    #[test]
    fn find_node_fast_consistent_with_find_node() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(10, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(20, NodeKind::Consumer, 60.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(30, NodeKind::Storage, 40.0, 50.0, 220.0));
        for id in [10, 20, 30, 99] {
            let slow = grid.find_node(id).map(|n| n.id);
            let fast = grid.find_node_fast(id).map(|n| n.id);
            assert_eq!(slow, fast, "Mismatch for node id {}", id);
        }
    }

    #[test]
    fn find_node_mut_updates_node() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        if let Some(n) = grid.find_node_mut(1) {
            n.set_output(42.0);
        }
        assert!((grid.find_node(1).unwrap().current_output_mw - 42.0).abs() < 1e-10);
    }

    #[test]
    fn storage_node_not_counted_as_gen_or_consumer() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Storage, 50.0, 50.0, 220.0));
        assert_eq!(grid.total_generation(), 0.0);
        assert_eq!(grid.total_consumption(), 0.0);
    }

    #[test]
    fn transmission_default_loss_fraction() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));
        grid.add_transmission(1, 2, 0.5, 200.0);
        assert!((grid.transmissions[0].loss_fraction - 0.02).abs() < 1e-10);
    }
}
