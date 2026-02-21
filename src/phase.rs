//! Phase synchronization and frequency event detection.

use crate::grid::PowerGrid;
use crate::node::NodeId;

#[inline(always)]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

/// Correction to be applied to a node's phase and frequency.
#[derive(Debug, Clone)]
pub struct PhaseCorrection {
    pub node_id: NodeId,
    pub correction_rad: f64,
    pub correction_hz: f64,
    pub timestamp_ns: u64,
}

/// Frequency deviation event.
#[derive(Debug, Clone)]
pub struct FrequencyEvent {
    pub timestamp_ns: u64,
    pub frequency_hz: f64,
    pub deviation_hz: f64,
    pub content_hash: u64,
}

/// Compute phase corrections to align all nodes with grid average.
pub fn compute_phase_corrections(grid: &PowerGrid) -> Vec<PhaseCorrection> {
    if grid.nodes.is_empty() { return Vec::new(); }

    let avg_phase: f64 = grid.nodes.iter().map(|n| n.phase_angle_rad).sum::<f64>()
                        / grid.nodes.len() as f64;

    grid.nodes.iter().map(|n| {
        let correction_rad = avg_phase - n.phase_angle_rad;
        PhaseCorrection {
            node_id: n.id,
            correction_rad,
            correction_hz: correction_rad * 0.1, // proportional correction
            timestamp_ns: grid.timestamp_ns,
        }
    }).collect()
}

/// Apply corrections to grid nodes.
pub fn apply_phase_corrections(grid: &mut PowerGrid, corrections: &[PhaseCorrection]) {
    for corr in corrections {
        if let Some(node) = grid.nodes.iter_mut().find(|n| n.id == corr.node_id) {
            node.phase_angle_rad += corr.correction_rad;
            node.frequency_hz += corr.correction_hz;
        }
    }
}

/// Maximum phase deviation from grid average.
pub fn max_phase_deviation(grid: &PowerGrid) -> f64 {
    if grid.nodes.is_empty() { return 0.0; }
    let avg: f64 = grid.nodes.iter().map(|n| n.phase_angle_rad).sum::<f64>()
                 / grid.nodes.len() as f64;
    grid.nodes.iter()
        .map(|n| (n.phase_angle_rad - avg).abs())
        .fold(0.0_f64, f64::max)
}

/// Check if all nodes are synchronized within tolerance.
pub fn is_synchronized(grid: &PowerGrid, tolerance_rad: f64) -> bool {
    max_phase_deviation(grid) <= tolerance_rad
}

/// Detect frequency anomaly if average deviation exceeds 0.1 Hz.
pub fn detect_frequency_event(grid: &PowerGrid, timestamp_ns: u64) -> Option<FrequencyEvent> {
    if grid.nodes.is_empty() { return None; }
    let avg_freq: f64 = grid.nodes.iter().map(|n| n.frequency_hz).sum::<f64>()
                       / grid.nodes.len() as f64;
    let deviation = avg_freq - grid.nominal_frequency_hz;
    if deviation.abs() > 0.1 {
        let mut hash_data = [0u8; 16];
        hash_data[0..8].copy_from_slice(&timestamp_ns.to_le_bytes());
        hash_data[8..16].copy_from_slice(&deviation.to_bits().to_le_bytes());
        Some(FrequencyEvent {
            timestamp_ns,
            frequency_hz: avg_freq,
            deviation_hz: deviation,
            content_hash: fnv1a(&hash_data),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{NodeKind, PowerNode};

    fn make_grid_with_phase_offsets(phases: &[f64]) -> PowerGrid {
        let mut grid = PowerGrid::new(1, 50.0);
        for (i, &phase) in phases.iter().enumerate() {
            let mut n = PowerNode::new(i as u64, NodeKind::Generator, 100.0, 50.0, 220.0);
            n.phase_angle_rad = phase;
            grid.add_node(n);
        }
        grid
    }

    #[test]
    fn corrections_on_misaligned() {
        let grid = make_grid_with_phase_offsets(&[0.0, 0.1, -0.1]);
        let corrs = compute_phase_corrections(&grid);
        assert_eq!(corrs.len(), 3);
    }

    #[test]
    fn apply_corrections_reduces_deviation() {
        let mut grid = make_grid_with_phase_offsets(&[0.0, 0.5, -0.5]);
        let dev_before = max_phase_deviation(&grid);
        let corrs = compute_phase_corrections(&grid);
        apply_phase_corrections(&mut grid, &corrs);
        let dev_after = max_phase_deviation(&grid);
        assert!(dev_after < dev_before);
    }

    #[test]
    fn synchronized_when_aligned() {
        let grid = make_grid_with_phase_offsets(&[0.0, 0.0, 0.0]);
        assert!(is_synchronized(&grid, 0.01));
    }

    #[test]
    fn not_synchronized_when_offset() {
        let grid = make_grid_with_phase_offsets(&[0.0, 1.0, -1.0]);
        assert!(!is_synchronized(&grid, 0.01));
    }

    #[test]
    fn frequency_event_detected() {
        let mut grid = PowerGrid::new(1, 50.0);
        let mut n = PowerNode::new(1, NodeKind::Generator, 100.0, 50.5, 220.0);
        n.frequency_hz = 50.5;
        grid.add_node(n);
        let ev = detect_frequency_event(&grid, 1000);
        assert!(ev.is_some());
        assert!(ev.unwrap().deviation_hz.abs() > 0.1);
    }

    #[test]
    fn no_frequency_event_when_stable() {
        let mut grid = PowerGrid::new(1, 50.0);
        grid.add_node(PowerNode::new(1, NodeKind::Generator, 100.0, 50.0, 220.0));
        assert!(detect_frequency_event(&grid, 1000).is_none());
    }
}
