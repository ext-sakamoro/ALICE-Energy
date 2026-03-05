//! Power grid node types.

/// Unique node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

/// Classification of power grid nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Generator,
    Consumer,
    Storage,
    Transformer,
    Relay,
}

/// A single node in the power grid.
#[derive(Debug, Clone)]
pub struct PowerNode {
    pub id: NodeId,
    pub kind: NodeKind,
    /// Maximum capacity in megawatts.
    pub capacity_mw: f64,
    /// Current output/consumption in megawatts.
    pub current_output_mw: f64,
    /// Grid frequency at this node (Hz).
    pub frequency_hz: f64,
    /// Voltage in kilovolts.
    pub voltage_kv: f64,
    /// Phase angle in radians.
    pub phase_angle_rad: f64,
}

impl PowerNode {
    #[must_use] 
    pub fn new(
        id: u64,
        kind: NodeKind,
        capacity_mw: f64,
        nominal_freq: f64,
        voltage_kv: f64,
    ) -> Self {
        Self {
            id: NodeId(id),
            kind,
            capacity_mw,
            current_output_mw: capacity_mw,
            frequency_hz: nominal_freq,
            voltage_kv,
            phase_angle_rad: 0.0,
        }
    }

    /// Utilization ratio (0.0 to 1.0).
    #[inline]
    #[must_use] 
    pub fn utilization(&self) -> f64 {
        if self.capacity_mw <= 0.0 {
            return 0.0;
        }
        (self.current_output_mw / self.capacity_mw).clamp(0.0, 1.0)
    }

    /// Set output, clamped to [0, capacity].
    #[inline]
    pub fn set_output(&mut self, mw: f64) {
        self.current_output_mw = mw.clamp(0.0, self.capacity_mw);
    }

    /// Whether current output exceeds capacity.
    #[inline]
    #[must_use] 
    pub fn is_overloaded(&self) -> bool {
        self.current_output_mw > self.capacity_mw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_generator() {
        let n = PowerNode::new(1, NodeKind::Generator, 500.0, 50.0, 220.0);
        assert_eq!(n.id, NodeId(1));
        assert_eq!(n.kind, NodeKind::Generator);
    }

    #[test]
    fn utilization() {
        let mut n = PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0);
        assert!((n.utilization() - 1.0).abs() < 1e-10);
        n.set_output(50.0);
        assert!((n.utilization() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn output_clamping() {
        let mut n = PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0);
        n.set_output(200.0);
        assert!((n.current_output_mw - 100.0).abs() < 1e-10);
        n.set_output(-50.0);
        assert!((n.current_output_mw - 0.0).abs() < 1e-10);
    }

    #[test]
    fn overload_detection() {
        let mut n = PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0);
        assert!(!n.is_overloaded());
        n.current_output_mw = 101.0; // bypass clamp
        assert!(n.is_overloaded());
    }

    #[test]
    fn node_kinds() {
        let kinds = [
            NodeKind::Generator,
            NodeKind::Consumer,
            NodeKind::Storage,
            NodeKind::Transformer,
            NodeKind::Relay,
        ];
        for k in &kinds {
            let n = PowerNode::new(1, *k, 50.0, 60.0, 110.0);
            assert_eq!(n.kind, *k);
        }
    }

    #[test]
    fn utilization_zero_capacity() {
        let n = PowerNode::new(1, NodeKind::Relay, 0.0, 50.0, 110.0);
        assert_eq!(n.utilization(), 0.0);
    }

    #[test]
    fn utilization_negative_capacity() {
        let n = PowerNode::new(1, NodeKind::Generator, -10.0, 50.0, 220.0);
        assert_eq!(n.utilization(), 0.0);
    }

    #[test]
    fn set_output_exact_capacity() {
        let mut n = PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0);
        n.set_output(100.0);
        assert!((n.current_output_mw - 100.0).abs() < 1e-10);
        assert!(!n.is_overloaded());
    }

    #[test]
    fn default_phase_angle_zero() {
        let n = PowerNode::new(42, NodeKind::Storage, 50.0, 60.0, 110.0);
        assert_eq!(n.phase_angle_rad, 0.0);
    }

    #[test]
    fn initial_output_equals_capacity() {
        let n = PowerNode::new(1, NodeKind::Generator, 250.0, 50.0, 220.0);
        assert!((n.current_output_mw - 250.0).abs() < 1e-10);
        assert!((n.utilization() - 1.0).abs() < 1e-10);
    }
}
