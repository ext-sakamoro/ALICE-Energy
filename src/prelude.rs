//! Convenience re-export (= `use alice_energy::prelude::*;` で主要 API 一括取得)
//!
//! Grid simulation 7 core module (grid / node / battery / dispatch / load_flow /
//! phase / renewable) の主要型 + 関数を prelude 経由で提供する
//! `contingency` / `facts` / `stability` は補助 module のため prelude 非対象

pub use crate::battery::{
    predict_degradation, time_to_replacement, BatteryChemistry, BatteryId, BatteryState,
};
pub use crate::dispatch::{economic_dispatch, DispatchConfig, DispatchResult, Generator};
pub use crate::grid::{GridId, PowerGrid, Transmission};
pub use crate::load_flow::{
    AcBusType, AcLoadFlow, AcLoadFlowConfig, AcLoadFlowResult, BusType, DcLoadFlow,
    DcLoadFlowConfig, DcLoadFlowResult,
};
pub use crate::node::{NodeId, NodeKind, PowerNode};
pub use crate::phase::{
    apply_phase_corrections, compute_phase_corrections, is_synchronized, max_phase_deviation,
    FrequencyEvent, PhaseCorrection,
};
pub use crate::renewable::{capacity_factor, solar_output, wind_output, SolarPanel, WindTurbine};
