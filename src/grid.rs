//! Power grid model.

use crate::node::{NodeId, NodeKind, PowerNode};

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
}

impl PowerGrid {
    pub fn new(id: u64, nominal_freq: f64) -> Self {
        Self {
            id: GridId(id),
            nodes: Vec::new(),
            transmissions: Vec::new(),
            nominal_frequency_hz: nominal_freq,
            timestamp_ns: 0,
        }
    }

    pub fn add_node(&mut self, node: PowerNode) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub fn add_transmission(&mut self, from: u64, to: u64, impedance: f64, max_capacity: f64) -> usize {
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
    pub fn total_generation(&self) -> f64 {
        self.nodes.iter()
            .filter(|n| n.kind == NodeKind::Generator)
            .map(|n| n.current_output_mw)
            .sum()
    }

    /// Total consumption (sum of Consumer outputs).
    pub fn total_consumption(&self) -> f64 {
        self.nodes.iter()
            .filter(|n| n.kind == NodeKind::Consumer)
            .map(|n| n.current_output_mw)
            .sum()
    }

    /// Supply minus demand (positive = surplus).
    #[inline]
    pub fn supply_demand_balance(&self) -> f64 {
        self.total_generation() - self.total_consumption()
    }

    /// Average frequency deviation from nominal.
    pub fn frequency_deviation(&self) -> f64 {
        if self.nodes.is_empty() { return 0.0; }
        let rcp_n = 1.0 / self.nodes.len() as f64;
        let avg: f64 = self.nodes.iter().map(|n| n.frequency_hz).sum::<f64>() * rcp_n;
        avg - self.nominal_frequency_hz
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }

    pub fn find_node(&self, id: u64) -> Option<&PowerNode> {
        self.nodes.iter().find(|n| n.id == NodeId(id))
    }

    pub fn find_node_mut(&mut self, id: u64) -> Option<&mut PowerNode> {
        self.nodes.iter_mut().find(|n| n.id == NodeId(id))
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
}
