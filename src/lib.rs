//! ALICE-Energy — Deterministic Power Grid Simulation
//!
//! Models power grids as deterministic physics simulations with microsecond-level
//! synchronization for supply/demand balancing, phase correction, and battery
//! degradation prediction.
//!
//! # Modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`battery`] | Battery state, chemistry models, degradation prediction |
//! | [`dispatch`] | Economic dispatch (merit-order generator scheduling) |
//! | [`grid`] | `PowerGrid` with supply/demand balance and transmission |
//! | [`load_flow`] | DC and AC load flow solvers (Gauss-Seidel, Newton-Raphson) |
//! | [`node`] | `PowerNode` with kind (Generator/Consumer/Storage/Renewable) |
//! | [`phase`] | Phase correction, frequency events, synchronization checks |
//! | [`renewable`] | Solar panel and wind turbine output models |
//!
//! # Quick Start
//!
//! ```
//! use alice_energy::{PowerGrid, PowerNode, NodeKind};
//!
//! let mut grid = PowerGrid::new(1, 50.0);
//! grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
//! grid.add_node(PowerNode::new(2, NodeKind::Consumer, 60.0, 50.0, 220.0));
//! assert!(grid.supply_demand_balance() > 0.0);
//! ```

pub mod battery;
pub mod dispatch;
pub mod grid;
pub mod load_flow;
pub mod node;
pub mod phase;
pub mod renewable;

pub use battery::{
    predict_degradation, time_to_replacement, BatteryChemistry, BatteryId, BatteryState,
};
pub use dispatch::{economic_dispatch, DispatchConfig, DispatchResult, Generator};
pub use grid::{GridId, PowerGrid, Transmission};
pub use load_flow::{
    AcBusType, AcLoadFlow, AcLoadFlowConfig, AcLoadFlowResult, BusType, DcLoadFlow,
    DcLoadFlowConfig, DcLoadFlowResult,
};
pub use node::{NodeId, NodeKind, PowerNode};
pub use phase::{
    apply_phase_corrections, compute_phase_corrections, is_synchronized, max_phase_deviation,
    FrequencyEvent, PhaseCorrection,
};
pub use renewable::{capacity_factor, solar_output, wind_output, SolarPanel, WindTurbine};

// ── Shared hash primitive ──────────────────────────────────────────────

#[inline(always)]
pub(crate) fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
