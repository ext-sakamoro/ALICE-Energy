//! ALICE-Energy — Deterministic Power Grid Simulation
//!
//! Models power grids as deterministic physics simulations with microsecond-level
//! synchronization for supply/demand balancing, phase correction, and battery
//! degradation prediction.
//!
//! ```
//! use alice_energy::{PowerGrid, PowerNode, NodeKind};
//!
//! let mut grid = PowerGrid::new(1, 50.0);
//! grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
//! grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));
//! assert!(grid.supply_demand_balance() > 0.0);
//! ```

pub mod node;
pub mod grid;
pub mod phase;
pub mod battery;

pub use node::{NodeId, NodeKind, PowerNode};
pub use grid::{GridId, Transmission, PowerGrid};
pub use phase::{PhaseCorrection, FrequencyEvent, compute_phase_corrections, apply_phase_corrections, max_phase_deviation, is_synchronized};
pub use battery::{BatteryId, BatteryChemistry, BatteryState, predict_degradation, time_to_replacement};
