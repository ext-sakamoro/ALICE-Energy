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
    let n = grid.nodes.len();
    if n == 0 { return Vec::new(); }

    // Pre-compute reciprocal to eliminate per-iteration division.
    let rcp_n = 1.0 / n as f64;
    let avg_phase: f64 = grid.nodes.iter().map(|n| n.phase_angle_rad).sum::<f64>() * rcp_n;

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
///
/// Uses an index-based direct write (O(n)) by iterating nodes in lock-step
/// with the corrections slice, avoiding the O(n^2) find-per-correction loop.
/// Corrections produced by `compute_phase_corrections` are guaranteed to be
/// in the same order as `grid.nodes`, so a single zip pass suffices.
/// Falls back to early-exit linear search only for out-of-order inputs.
pub fn apply_phase_corrections(grid: &mut PowerGrid, corrections: &[PhaseCorrection]) {
    if corrections.is_empty() { return; }

    // Fast path: corrections produced by compute_phase_corrections are in
    // the same order as grid.nodes.  Zip and apply in O(n).
    let in_order = corrections.len() == grid.nodes.len()
        && corrections.iter().zip(grid.nodes.iter()).all(|(c, n)| c.node_id == n.id);

    if in_order {
        for (node, corr) in grid.nodes.iter_mut().zip(corrections.iter()) {
            node.phase_angle_rad += corr.correction_rad;
            node.frequency_hz += corr.correction_hz;
        }
        return;
    }

    // Slow path (arbitrary correction order): iterate corrections; for each
    // correction scan nodes once but break as soon as the match is found,
    // giving O(n) average-case early-exit behaviour.
    for corr in corrections {
        for node in grid.nodes.iter_mut() {
            if node.id == corr.node_id {
                node.phase_angle_rad += corr.correction_rad;
                node.frequency_hz += corr.correction_hz;
                break; // early-exit: node ids are unique
            }
        }
    }
}

/// Maximum phase deviation from grid average.
pub fn max_phase_deviation(grid: &PowerGrid) -> f64 {
    let n = grid.nodes.len();
    if n == 0 { return 0.0; }
    // Pre-compute reciprocal to eliminate per-call division.
    let rcp_n = 1.0 / n as f64;
    let avg: f64 = grid.nodes.iter().map(|n| n.phase_angle_rad).sum::<f64>() * rcp_n;
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
    let n = grid.nodes.len();
    if n == 0 { return None; }
    // Pre-compute reciprocal to eliminate the division inside the hot sum.
    let rcp_n = 1.0 / n as f64;
    let avg_freq: f64 = grid.nodes.iter().map(|n| n.frequency_hz).sum::<f64>() * rcp_n;
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

    /// Verify the O(n) fast path: corrections in node order are applied correctly.
    #[test]
    fn apply_corrections_fast_path() {
        let mut grid = make_grid_with_phase_offsets(&[0.1, 0.2, 0.3]);
        let corrs = compute_phase_corrections(&grid);
        apply_phase_corrections(&mut grid, &corrs);
        // After correction all nodes should be at the original average.
        let avg = (0.1 + 0.2 + 0.3) / 3.0;
        for node in &grid.nodes {
            assert!((node.phase_angle_rad - avg).abs() < 1e-12);
        }
    }

    /// Verify the slow path (out-of-order corrections) also produces correct results.
    #[test]
    fn apply_corrections_slow_path_out_of_order() {
        let phases = [0.0_f64, 0.6, -0.3];
        let mut grid = make_grid_with_phase_offsets(&phases);
        let mut corrs = compute_phase_corrections(&grid);
        // Reverse to force the slow path (ids no longer match node order).
        corrs.reverse();
        apply_phase_corrections(&mut grid, &corrs);
        let dev_after = max_phase_deviation(&grid);
        assert!(dev_after < 1e-12);
    }
}
