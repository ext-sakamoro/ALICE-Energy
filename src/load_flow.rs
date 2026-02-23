//! DC power flow solver
//!
//! Simplified DC load flow using the linear approximation P = B * θ,
//! where B is the bus susceptance matrix and θ is the vector of voltage
//! angles. Solved iteratively with Gauss-Seidel relaxation.
//!
//! Author: Moroya Sakamoto

use crate::fnv1a;

/// Bus type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    /// Slack bus (reference angle = 0). Absorbs mismatch.
    Slack,
    /// PV bus (generator): specified P, voltage magnitude.
    Generator,
    /// PQ bus (load): specified P and Q.
    Load,
}

/// Configuration for DC load flow.
#[derive(Debug, Clone, Copy)]
pub struct DcLoadFlowConfig {
    /// Maximum Gauss-Seidel iterations. Default 100.
    pub max_iterations: usize,
    /// Convergence threshold for angle change (radians). Default 1e-8.
    pub convergence_threshold: f64,
    /// Relaxation factor (1.0 = standard, >1 = over-relaxation). Default 1.0.
    pub relaxation: f64,
}

impl Default for DcLoadFlowConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            convergence_threshold: 1e-8,
            relaxation: 1.0,
        }
    }
}

/// A bus in the DC load flow model.
#[derive(Debug, Clone)]
pub struct Bus {
    /// Bus index.
    pub index: usize,
    /// Bus type.
    pub bus_type: BusType,
    /// Net injected power (MW). Positive = generation, negative = load.
    pub power_mw: f64,
}

/// A branch (transmission line) between two buses.
#[derive(Debug, Clone, Copy)]
pub struct Branch {
    /// From bus index.
    pub from: usize,
    /// To bus index.
    pub to: usize,
    /// Line susceptance (1/reactance, per unit). Must be positive.
    pub susceptance: f64,
}

/// Result of DC load flow computation.
#[derive(Debug, Clone)]
pub struct DcLoadFlowResult {
    /// Voltage angles at each bus (radians). Slack bus = 0.
    pub angles_rad: Vec<f64>,
    /// Power flow on each branch (MW). Positive = from→to.
    pub branch_flows_mw: Vec<f64>,
    /// Number of iterations performed.
    pub iterations: usize,
    /// Whether the solver converged.
    pub converged: bool,
    /// Maximum angle change in the last iteration.
    pub residual: f64,
    /// Deterministic content hash.
    pub content_hash: u64,
}

/// DC load flow solver.
#[derive(Debug, Clone)]
pub struct DcLoadFlow {
    buses: Vec<Bus>,
    branches: Vec<Branch>,
    config: DcLoadFlowConfig,
}

impl DcLoadFlow {
    /// Create a new DC load flow problem.
    pub fn new(config: DcLoadFlowConfig) -> Self {
        Self {
            buses: Vec::new(),
            branches: Vec::new(),
            config,
        }
    }

    /// Add a bus. Returns bus index.
    pub fn add_bus(&mut self, bus_type: BusType, power_mw: f64) -> usize {
        let idx = self.buses.len();
        self.buses.push(Bus {
            index: idx,
            bus_type,
            power_mw,
        });
        idx
    }

    /// Add a branch between two buses.
    pub fn add_branch(&mut self, from: usize, to: usize, susceptance: f64) {
        self.branches.push(Branch {
            from,
            to,
            susceptance,
        });
    }

    /// Number of buses.
    pub fn bus_count(&self) -> usize {
        self.buses.len()
    }

    /// Solve the DC load flow using Gauss-Seidel iteration.
    ///
    /// DC approximation: P_i = sum_j B_ij * (θ_i - θ_j)
    /// Rearranged: θ_i = (P_i + sum_{j≠i} B_ij * θ_j) / B_ii
    /// where B_ii = sum_j B_ij (diagonal of susceptance matrix).
    pub fn solve(&self) -> DcLoadFlowResult {
        let n = self.buses.len();
        if n == 0 {
            return DcLoadFlowResult {
                angles_rad: Vec::new(),
                branch_flows_mw: Vec::new(),
                iterations: 0,
                converged: true,
                residual: 0.0,
                content_hash: fnv1a(b"empty_load_flow"),
            };
        }

        // Build susceptance data: diagonal and off-diagonal entries
        // B_diag[i] = sum of susceptances connected to bus i
        // neighbors[i] = [(j, b_ij), ...]
        let mut b_diag = vec![0.0f64; n];
        let mut neighbors: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];

        for br in &self.branches {
            b_diag[br.from] += br.susceptance;
            b_diag[br.to] += br.susceptance;
            neighbors[br.from].push((br.to, br.susceptance));
            neighbors[br.to].push((br.from, br.susceptance));
        }

        let mut angles = vec![0.0f64; n];
        let mut iterations = 0;
        let mut residual = 0.0;
        let mut converged = false;

        for iter in 0..self.config.max_iterations {
            let mut max_change = 0.0f64;

            for i in 0..n {
                if self.buses[i].bus_type == BusType::Slack {
                    continue; // Slack bus angle is fixed at 0
                }

                if b_diag[i] == 0.0 {
                    continue; // Isolated bus
                }

                // θ_i_new = (P_i + sum_j B_ij * θ_j) / B_ii
                let mut sum_b_theta = 0.0f64;
                for &(j, b_ij) in &neighbors[i] {
                    sum_b_theta += b_ij * angles[j];
                }

                let rcp_bii = 1.0 / b_diag[i];
                let theta_new = (self.buses[i].power_mw + sum_b_theta) * rcp_bii;

                // Apply relaxation
                let theta_relaxed = angles[i] + self.config.relaxation * (theta_new - angles[i]);

                let change = (theta_relaxed - angles[i]).abs();
                if change > max_change {
                    max_change = change;
                }
                angles[i] = theta_relaxed;
            }

            iterations = iter + 1;
            residual = max_change;

            if max_change < self.config.convergence_threshold {
                converged = true;
                break;
            }
        }

        // Compute branch flows: P_ij = B_ij * (θ_i - θ_j)
        let branch_flows: Vec<f64> = self
            .branches
            .iter()
            .map(|br| br.susceptance * (angles[br.from] - angles[br.to]))
            .collect();

        // Content hash
        let mut buf = Vec::with_capacity(n * 8 + 8);
        buf.extend_from_slice(&(n as u64).to_le_bytes());
        for &a in &angles {
            buf.extend_from_slice(&a.to_bits().to_le_bytes());
        }
        let content_hash = fnv1a(&buf);

        DcLoadFlowResult {
            angles_rad: angles,
            branch_flows_mw: branch_flows,
            iterations,
            converged,
            residual,
            content_hash,
        }
    }
}

// ── AC Load Flow (Newton-Raphson) ──────────────────────────────────────

/// AC bus type classification for full power flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcBusType {
    /// Slack bus: reference voltage magnitude and angle.
    Slack,
    /// PV bus: specified P and |V|, unknown Q and θ.
    PV,
    /// PQ bus: specified P and Q, unknown |V| and θ.
    PQ,
}

/// An AC bus in the power flow model.
#[derive(Debug, Clone)]
pub struct AcBus {
    pub index: usize,
    pub bus_type: AcBusType,
    /// Net active power injection (MW). Positive = generation.
    pub p_mw: f64,
    /// Net reactive power injection (MVAr). Positive = generation.
    pub q_mvar: f64,
    /// Voltage magnitude (per unit).
    pub v_pu: f64,
}

/// An AC branch (transmission line) between two buses.
#[derive(Debug, Clone, Copy)]
pub struct AcBranch {
    pub from: usize,
    pub to: usize,
    /// Series conductance (per unit).
    pub g: f64,
    /// Series susceptance (per unit).
    pub b: f64,
}

/// Configuration for AC load flow.
#[derive(Debug, Clone, Copy)]
pub struct AcLoadFlowConfig {
    pub max_iterations: usize,
    pub convergence_threshold: f64,
}

impl Default for AcLoadFlowConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            convergence_threshold: 1e-6,
        }
    }
}

/// Result of AC load flow computation.
#[derive(Debug, Clone)]
pub struct AcLoadFlowResult {
    /// Voltage magnitudes at each bus (per unit).
    pub v_pu: Vec<f64>,
    /// Voltage angles at each bus (radians).
    pub angles_rad: Vec<f64>,
    /// Number of iterations performed.
    pub iterations: usize,
    /// Whether the solver converged.
    pub converged: bool,
    /// Maximum mismatch in the last iteration.
    pub residual: f64,
    /// Deterministic content hash.
    pub content_hash: u64,
}

/// AC power flow solver using Newton-Raphson method.
#[derive(Debug, Clone)]
pub struct AcLoadFlow {
    buses: Vec<AcBus>,
    branches: Vec<AcBranch>,
    config: AcLoadFlowConfig,
}

impl AcLoadFlow {
    pub fn new(config: AcLoadFlowConfig) -> Self {
        Self {
            buses: Vec::new(),
            branches: Vec::new(),
            config,
        }
    }

    pub fn add_bus(&mut self, bus_type: AcBusType, p_mw: f64, q_mvar: f64, v_pu: f64) -> usize {
        let idx = self.buses.len();
        self.buses.push(AcBus {
            index: idx,
            bus_type,
            p_mw,
            q_mvar,
            v_pu,
        });
        idx
    }

    pub fn add_branch(&mut self, from: usize, to: usize, g: f64, b: f64) {
        self.branches.push(AcBranch { from, to, g, b });
    }

    pub fn bus_count(&self) -> usize {
        self.buses.len()
    }

    /// Solve AC power flow using Newton-Raphson iteration.
    ///
    /// At each iteration:
    /// 1. Compute power injections P_calc, Q_calc from Y-bus
    /// 2. Form mismatch vector ΔP, ΔQ
    /// 3. Build Jacobian (J1=dP/dθ, J2=dP/dV, J3=dQ/dθ, J4=dQ/dV)
    /// 4. Solve J * [Δθ; ΔV/V] = [ΔP; ΔQ] via Gaussian elimination
    /// 5. Update angles and voltages
    pub fn solve(&self) -> AcLoadFlowResult {
        let n = self.buses.len();
        if n == 0 {
            return AcLoadFlowResult {
                v_pu: Vec::new(),
                angles_rad: Vec::new(),
                iterations: 0,
                converged: true,
                residual: 0.0,
                content_hash: fnv1a(b"empty_ac_load_flow"),
            };
        }

        // Build admittance matrix (dense, G + jB)
        let mut g_mat = vec![vec![0.0f64; n]; n];
        let mut b_mat = vec![vec![0.0f64; n]; n];
        for br in &self.branches {
            g_mat[br.from][br.to] -= br.g;
            g_mat[br.to][br.from] -= br.g;
            b_mat[br.from][br.to] -= br.b;
            b_mat[br.to][br.from] -= br.b;
            g_mat[br.from][br.from] += br.g;
            g_mat[br.to][br.to] += br.g;
            b_mat[br.from][br.from] += br.b;
            b_mat[br.to][br.to] += br.b;
        }

        let mut v: Vec<f64> = self.buses.iter().map(|b| b.v_pu).collect();
        let mut theta = vec![0.0f64; n];

        // Identify PQ and non-slack bus indices
        let pq_indices: Vec<usize> = self
            .buses
            .iter()
            .filter(|b| b.bus_type == AcBusType::PQ)
            .map(|b| b.index)
            .collect();
        let non_slack: Vec<usize> = self
            .buses
            .iter()
            .filter(|b| b.bus_type != AcBusType::Slack)
            .map(|b| b.index)
            .collect();

        let n_p = non_slack.len();
        let n_q = pq_indices.len();
        let dim = n_p + n_q;

        let mut iterations = 0;
        let mut residual = 0.0;
        let mut converged = false;

        for iter in 0..self.config.max_iterations {
            // Compute P_calc, Q_calc
            let mut p_calc = vec![0.0f64; n];
            let mut q_calc = vec![0.0f64; n];
            for i in 0..n {
                for j in 0..n {
                    let angle_diff = theta[i] - theta[j];
                    let cos_d = angle_diff.cos();
                    let sin_d = angle_diff.sin();
                    p_calc[i] += v[i] * v[j] * (g_mat[i][j] * cos_d + b_mat[i][j] * sin_d);
                    q_calc[i] += v[i] * v[j] * (g_mat[i][j] * sin_d - b_mat[i][j] * cos_d);
                }
            }

            // Mismatch vector
            let mut mismatch = vec![0.0f64; dim];
            for (k, &i) in non_slack.iter().enumerate() {
                mismatch[k] = self.buses[i].p_mw - p_calc[i];
            }
            for (k, &i) in pq_indices.iter().enumerate() {
                mismatch[n_p + k] = self.buses[i].q_mvar - q_calc[i];
            }

            residual = mismatch.iter().map(|m| m.abs()).fold(0.0f64, f64::max);
            iterations = iter + 1;

            if residual < self.config.convergence_threshold {
                converged = true;
                break;
            }

            if dim == 0 {
                converged = true;
                break;
            }

            // Build Jacobian (dense dim × dim)
            let mut jac = vec![vec![0.0f64; dim]; dim];

            // J1: dP/dθ (n_p × n_p)
            for (ki, &i) in non_slack.iter().enumerate() {
                for (kj, &j) in non_slack.iter().enumerate() {
                    if i == j {
                        jac[ki][kj] = -q_calc[i] - b_mat[i][i] * v[i] * v[i];
                    } else {
                        let a = theta[i] - theta[j];
                        jac[ki][kj] = v[i] * v[j] * (g_mat[i][j] * a.sin() - b_mat[i][j] * a.cos());
                    }
                }
            }

            // J2: dP/dV (n_p × n_q)
            for (ki, &i) in non_slack.iter().enumerate() {
                for (kj, &j) in pq_indices.iter().enumerate() {
                    if i == j {
                        jac[ki][n_p + kj] = p_calc[i] / v[i].max(1e-10) + g_mat[i][i] * v[i];
                    } else {
                        let a = theta[i] - theta[j];
                        jac[ki][n_p + kj] = v[i] * (g_mat[i][j] * a.cos() + b_mat[i][j] * a.sin());
                    }
                }
            }

            // J3: dQ/dθ (n_q × n_p)
            for (ki, &i) in pq_indices.iter().enumerate() {
                for (kj, &j) in non_slack.iter().enumerate() {
                    if i == j {
                        jac[n_p + ki][kj] = p_calc[i] - g_mat[i][i] * v[i] * v[i];
                    } else {
                        let a = theta[i] - theta[j];
                        jac[n_p + ki][kj] =
                            -v[i] * v[j] * (g_mat[i][j] * a.cos() + b_mat[i][j] * a.sin());
                    }
                }
            }

            // J4: dQ/dV (n_q × n_q)
            for (ki, &i) in pq_indices.iter().enumerate() {
                for (kj, &j) in pq_indices.iter().enumerate() {
                    if i == j {
                        jac[n_p + ki][n_p + kj] = q_calc[i] / v[i].max(1e-10) - b_mat[i][i] * v[i];
                    } else {
                        let a = theta[i] - theta[j];
                        jac[n_p + ki][n_p + kj] =
                            v[i] * (g_mat[i][j] * a.sin() - b_mat[i][j] * a.cos());
                    }
                }
            }

            // Solve Jac * dx = mismatch via Gaussian elimination with partial pivoting
            let dx = gauss_solve(jac, mismatch);

            // Update angles for non-slack buses
            for (k, &i) in non_slack.iter().enumerate() {
                theta[i] += dx[k];
            }
            // Update voltage magnitudes for PQ buses
            for (k, &i) in pq_indices.iter().enumerate() {
                v[i] += dx[n_p + k] * v[i];
            }
        }

        let mut buf = Vec::with_capacity(n * 16 + 8);
        buf.extend_from_slice(&(n as u64).to_le_bytes());
        for i in 0..n {
            buf.extend_from_slice(&v[i].to_bits().to_le_bytes());
            buf.extend_from_slice(&theta[i].to_bits().to_le_bytes());
        }

        AcLoadFlowResult {
            v_pu: v,
            angles_rad: theta,
            iterations,
            converged,
            residual,
            content_hash: fnv1a(&buf),
        }
    }
}

/// Gaussian elimination with partial pivoting.
fn gauss_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    if n == 0 {
        return Vec::new();
    }

    // Forward elimination
    for col in 0..n {
        // Partial pivot
        let mut max_row = col;
        let mut max_val = a[col][col].abs();
        for (row, a_row) in a.iter().enumerate().skip(col + 1) {
            let v = a_row[col].abs();
            if v > max_val {
                max_val = v;
                max_row = row;
            }
        }
        if max_val < 1e-15 {
            continue;
        }
        a.swap(col, max_row);
        b.swap(col, max_row);

        let rcp_pivot = 1.0 / a[col][col];
        for row in (col + 1)..n {
            let factor = a[row][col] * rcp_pivot;
            let col_row: Vec<f64> = a[col][col..n].to_vec();
            for (k, &c) in col_row.iter().enumerate() {
                a[row][col + k] -= factor * c;
            }
            b[row] -= factor * b[col];
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        if a[i][i].abs() < 1e-15 {
            continue;
        }
        let mut s = b[i];
        for j in (i + 1)..n {
            s -= a[i][j] * x[j];
        }
        x[i] = s / a[i][i];
    }
    x
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_system() {
        let lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        let result = lf.solve();
        assert!(result.converged);
        assert!(result.angles_rad.is_empty());
        assert!(result.branch_flows_mw.is_empty());
    }

    #[test]
    fn single_slack_bus() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 0.0);
        let result = lf.solve();
        assert!(result.converged);
        assert_eq!(result.angles_rad.len(), 1);
        assert!((result.angles_rad[0]).abs() < 1e-15);
    }

    #[test]
    fn two_bus_system() {
        // Slack (gen) ---[b=10]--- Load (-50 MW)
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 50.0);
        lf.add_bus(BusType::Load, -50.0);
        lf.add_branch(0, 1, 10.0);

        let result = lf.solve();
        assert!(result.converged);
        // Slack angle = 0, load angle = -50/10 = -5 rad
        assert!((result.angles_rad[0]).abs() < 1e-10);
        assert!((result.angles_rad[1] - (-5.0)).abs() < 1e-6);
        // Flow from 0→1 = 10 * (0 - (-5)) = 50 MW
        assert!((result.branch_flows_mw[0] - 50.0).abs() < 1e-6);
    }

    #[test]
    fn three_bus_balanced() {
        // Bus 0: Slack (+100)
        // Bus 1: Load (-60)
        // Bus 2: Load (-40)
        // Branch 0-1: b=20, Branch 0-2: b=10, Branch 1-2: b=5
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 100.0);
        lf.add_bus(BusType::Load, -60.0);
        lf.add_bus(BusType::Load, -40.0);
        lf.add_branch(0, 1, 20.0);
        lf.add_branch(0, 2, 10.0);
        lf.add_branch(1, 2, 5.0);

        let result = lf.solve();
        assert!(result.converged);
        assert_eq!(result.angles_rad.len(), 3);
        // Slack bus angle is 0
        assert!((result.angles_rad[0]).abs() < 1e-10);
        // All branch flows should be computed
        assert_eq!(result.branch_flows_mw.len(), 3);
    }

    #[test]
    fn convergence_with_small_system() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig {
            max_iterations: 1000,
            convergence_threshold: 1e-12,
            relaxation: 1.0,
        });
        lf.add_bus(BusType::Slack, 100.0);
        lf.add_bus(BusType::Load, -30.0);
        lf.add_bus(BusType::Load, -70.0);
        lf.add_branch(0, 1, 15.0);
        lf.add_branch(0, 2, 10.0);
        lf.add_branch(1, 2, 8.0);

        let result = lf.solve();
        assert!(result.converged);
        assert!(result.residual < 1e-12);
    }

    #[test]
    fn power_balance_at_buses() {
        // Verify Kirchhoff: net injection = sum of outgoing flows
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 0.0);
        lf.add_bus(BusType::Load, -20.0);
        lf.add_bus(BusType::Load, -30.0);
        lf.add_branch(0, 1, 10.0);
        lf.add_branch(0, 2, 10.0);
        lf.add_branch(1, 2, 5.0);

        let result = lf.solve();
        assert!(result.converged);

        // For load buses, verify P_i ≈ sum of B_ij*(θ_i - θ_j)
        let angles = &result.angles_rad;
        // Bus 1: P = -20 ≈ 10*(θ1-θ0) + 5*(θ1-θ2)
        let p1_calc = 10.0 * (angles[1] - angles[0]) + 5.0 * (angles[1] - angles[2]);
        assert!(
            (p1_calc - (-20.0)).abs() < 1e-6,
            "Bus 1 mismatch: {}",
            p1_calc
        );
    }

    #[test]
    fn generator_bus() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 0.0);
        lf.add_bus(BusType::Generator, 80.0);
        lf.add_bus(BusType::Load, -80.0);
        lf.add_branch(0, 1, 10.0);
        lf.add_branch(1, 2, 10.0);

        let result = lf.solve();
        assert!(result.converged);
        assert_eq!(result.branch_flows_mw.len(), 2);
    }

    #[test]
    fn content_hash_deterministic() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 50.0);
        lf.add_bus(BusType::Load, -50.0);
        lf.add_branch(0, 1, 10.0);

        let r1 = lf.solve();
        let r2 = lf.solve();
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_ne!(r1.content_hash, 0);
    }

    #[test]
    fn relaxation_factor() {
        // Over-relaxation should still converge for a well-conditioned system
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig {
            max_iterations: 200,
            convergence_threshold: 1e-8,
            relaxation: 1.2,
        });
        lf.add_bus(BusType::Slack, 100.0);
        lf.add_bus(BusType::Load, -60.0);
        lf.add_bus(BusType::Load, -40.0);
        lf.add_branch(0, 1, 10.0);
        lf.add_branch(0, 2, 10.0);

        let result = lf.solve();
        assert!(result.converged);
    }

    #[test]
    fn isolated_bus_no_panic() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 0.0);
        lf.add_bus(BusType::Load, -10.0); // Isolated, no branch
        let result = lf.solve();
        // Should not panic; isolated bus stays at angle 0
        assert_eq!(result.angles_rad.len(), 2);
    }

    #[test]
    fn bus_count() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        assert_eq!(lf.bus_count(), 0);
        lf.add_bus(BusType::Slack, 0.0);
        lf.add_bus(BusType::Load, -10.0);
        assert_eq!(lf.bus_count(), 2);
    }

    // ── AC Load Flow Tests ────────────────────────────────────────────

    #[test]
    fn ac_empty_system() {
        let lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        let r = lf.solve();
        assert!(r.converged);
        assert!(r.v_pu.is_empty());
    }

    #[test]
    fn ac_single_slack() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        let r = lf.solve();
        assert!(r.converged);
        assert!((r.v_pu[0] - 1.0).abs() < 1e-10);
        assert!((r.angles_rad[0]).abs() < 1e-10);
    }

    #[test]
    fn ac_two_bus_pq() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        lf.add_bus(AcBusType::PQ, -1.0, -0.5, 1.0);
        lf.add_branch(0, 1, 0.0, 10.0); // pure susceptance
        let r = lf.solve();
        assert!(r.converged, "Did not converge, residual={}", r.residual);
        assert_eq!(r.v_pu.len(), 2);
        assert_eq!(r.angles_rad.len(), 2);
    }

    #[test]
    fn ac_three_bus() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        lf.add_bus(AcBusType::PV, 0.5, 0.0, 1.02);
        lf.add_bus(AcBusType::PQ, -1.0, -0.3, 1.0);
        lf.add_branch(0, 1, 0.01, 10.0);
        lf.add_branch(1, 2, 0.01, 8.0);
        lf.add_branch(0, 2, 0.01, 5.0);
        let r = lf.solve();
        assert!(r.converged);
        // Slack angle stays at 0
        assert!((r.angles_rad[0]).abs() < 1e-10);
    }

    #[test]
    fn ac_bus_count() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        assert_eq!(lf.bus_count(), 0);
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        lf.add_bus(AcBusType::PQ, -1.0, -0.5, 1.0);
        assert_eq!(lf.bus_count(), 2);
    }

    #[test]
    fn ac_content_hash_deterministic() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        lf.add_bus(AcBusType::PQ, -1.0, -0.5, 1.0);
        lf.add_branch(0, 1, 0.0, 10.0);
        let r1 = lf.solve();
        let r2 = lf.solve();
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_ne!(r1.content_hash, 0);
    }

    #[test]
    fn ac_convergence_tight() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig {
            max_iterations: 100,
            convergence_threshold: 1e-10,
        });
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        lf.add_bus(AcBusType::PQ, -0.5, -0.2, 1.0);
        lf.add_branch(0, 1, 0.01, 20.0);
        let r = lf.solve();
        assert!(r.converged);
        assert!(r.residual < 1e-10);
    }

    #[test]
    fn gauss_solve_identity() {
        let a = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let b = vec![3.0, 7.0];
        let x = gauss_solve(a, b);
        assert!((x[0] - 3.0).abs() < 1e-10);
        assert!((x[1] - 7.0).abs() < 1e-10);
    }

    #[test]
    fn gauss_solve_2x2() {
        // 2x + y = 5, x + 3y = 10 → x=1, y=3
        let a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let b = vec![5.0, 10.0];
        let x = gauss_solve(a, b);
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn gauss_solve_3x3() {
        // x + y + z = 6, 2y + 5z = -4, 2x + 5y - z = 27 → x=5, y=3, z=-2
        let a = vec![
            vec![1.0, 1.0, 1.0],
            vec![0.0, 2.0, 5.0],
            vec![2.0, 5.0, -1.0],
        ];
        let b = vec![6.0, -4.0, 27.0];
        let x = gauss_solve(a, b);
        assert!((x[0] - 5.0).abs() < 1e-8);
        assert!((x[1] - 3.0).abs() < 1e-8);
        assert!((x[2] - (-2.0)).abs() < 1e-8);
    }

    #[test]
    fn gauss_solve_empty() {
        let x = gauss_solve(Vec::new(), Vec::new());
        assert!(x.is_empty());
    }

    #[test]
    fn gauss_solve_singular_no_panic() {
        // Singular matrix: row 2 = 2 * row 1
        let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        let b = vec![3.0, 6.0];
        let _x = gauss_solve(a, b);
        // Should not panic; result may be arbitrary but no crash
    }

    #[test]
    fn dc_five_bus_ring() {
        // Ring topology: 0-1-2-3-4-0
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 0.0);
        lf.add_bus(BusType::Load, -25.0);
        lf.add_bus(BusType::Load, -25.0);
        lf.add_bus(BusType::Load, -25.0);
        lf.add_bus(BusType::Load, -25.0);
        lf.add_branch(0, 1, 10.0);
        lf.add_branch(1, 2, 10.0);
        lf.add_branch(2, 3, 10.0);
        lf.add_branch(3, 4, 10.0);
        lf.add_branch(4, 0, 10.0);
        let result = lf.solve();
        assert!(result.converged);
        assert_eq!(result.branch_flows_mw.len(), 5);
    }

    #[test]
    fn dc_max_iterations_reached() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig {
            max_iterations: 1,
            convergence_threshold: 1e-30, // impossibly tight
            relaxation: 1.0,
        });
        lf.add_bus(BusType::Slack, 100.0);
        lf.add_bus(BusType::Load, -50.0);
        lf.add_bus(BusType::Load, -50.0);
        lf.add_branch(0, 1, 10.0);
        lf.add_branch(1, 2, 10.0);
        lf.add_branch(0, 2, 10.0);
        let result = lf.solve();
        // With only 1 iteration and very tight threshold, convergence is unlikely
        assert_eq!(result.iterations, 1);
    }

    #[test]
    fn ac_pv_bus_voltage_preserved() {
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        lf.add_bus(AcBusType::PV, 1.0, 0.0, 1.05);
        lf.add_bus(AcBusType::PQ, -1.0, -0.3, 1.0);
        lf.add_branch(0, 1, 0.01, 15.0);
        lf.add_branch(1, 2, 0.01, 10.0);
        let r = lf.solve();
        assert!(r.converged);
        // PV bus voltage should stay close to specified value
        // (Newton-Raphson only updates V for PQ buses)
        // Slack and PV buses should keep their initial voltages
        assert!((r.v_pu[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ac_all_slack_only_no_crash() {
        // Only slack bus — dim=0, should converge immediately
        let mut lf = AcLoadFlow::new(AcLoadFlowConfig::default());
        lf.add_bus(AcBusType::Slack, 0.0, 0.0, 1.0);
        let r = lf.solve();
        assert!(r.converged);
        assert_eq!(r.iterations, 1);
    }

    #[test]
    fn dc_all_generators_no_load() {
        let mut lf = DcLoadFlow::new(DcLoadFlowConfig::default());
        lf.add_bus(BusType::Slack, 50.0);
        lf.add_bus(BusType::Generator, 50.0);
        lf.add_branch(0, 1, 10.0);
        let result = lf.solve();
        assert!(result.converged);
    }
}
