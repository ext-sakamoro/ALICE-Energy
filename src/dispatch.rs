//! Economic dispatch via lambda iteration
//!
//! Optimally allocates generation among multiple generators to minimize
//! total fuel cost subject to power balance constraints. Uses the
//! equal incremental cost (lambda iteration) method.
//!
//! Cost function: `C_i(P)` = `a_i` + `b_i` * P + `c_i` * P²
//! Optimal condition: `dC_i/dP` = `b_i` + 2*`c_i`*P = λ for all generators
//!
//! Author: Moroya Sakamoto

use crate::fnv1a;

/// A generator with quadratic cost function.
///
/// Cost = a + b*P + c*P² ($/h)
/// Incremental cost = dC/dP = b + 2*c*P ($/`MWh`)
#[derive(Debug, Clone, Copy)]
pub struct Generator {
    /// Generator identifier.
    pub id: u64,
    /// Constant cost term ($/h).
    pub cost_a: f64,
    /// Linear cost coefficient ($/`MWh`).
    pub cost_b: f64,
    /// Quadratic cost coefficient ($/(`MW²h`)).
    pub cost_c: f64,
    /// Minimum output (MW).
    pub p_min: f64,
    /// Maximum output (MW).
    pub p_max: f64,
}

impl Generator {
    /// Create a new generator.
    #[must_use]
    pub const fn new(
        id: u64,
        cost_a: f64,
        cost_b: f64,
        cost_c: f64,
        p_min: f64,
        p_max: f64,
    ) -> Self {
        Self {
            id,
            cost_a,
            cost_b,
            cost_c,
            p_min,
            p_max,
        }
    }

    /// Cost at given power output.
    #[inline]
    #[must_use]
    pub fn cost(&self, p: f64) -> f64 {
        (self.cost_c * p).mul_add(p, self.cost_b.mul_add(p, self.cost_a))
    }

    /// Incremental cost (dC/dP) at given power output.
    #[inline]
    #[must_use]
    pub fn incremental_cost(&self, p: f64) -> f64 {
        (2.0 * self.cost_c).mul_add(p, self.cost_b)
    }

    /// Optimal output for a given lambda (system incremental cost).
    /// P = (λ - b) / (2c), clamped to [`p_min`, `p_max`].
    #[inline]
    #[must_use]
    pub fn optimal_output(&self, lambda: f64) -> f64 {
        if self.cost_c <= 0.0 {
            // Linear cost: run at max if lambda >= b, else min
            return if lambda >= self.cost_b {
                self.p_max
            } else {
                self.p_min
            };
        }
        let rcp_2c = 0.5 / self.cost_c;
        let p_opt = (lambda - self.cost_b) * rcp_2c;
        p_opt.clamp(self.p_min, self.p_max)
    }
}

/// Configuration for economic dispatch.
#[derive(Debug, Clone, Copy)]
pub struct DispatchConfig {
    /// Maximum iterations for lambda search. Default 100.
    pub max_iterations: usize,
    /// Power mismatch tolerance (MW). Default 0.01.
    pub tolerance_mw: f64,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance_mw: 0.01,
        }
    }
}

/// Result of economic dispatch.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// Optimal output for each generator (MW), same order as input.
    pub outputs_mw: Vec<f64>,
    /// Total generation (MW).
    pub total_generation_mw: f64,
    /// Total cost ($/h).
    pub total_cost: f64,
    /// System lambda (marginal cost, $/`MWh`).
    pub lambda: f64,
    /// Whether the solver converged.
    pub converged: bool,
    /// Number of iterations.
    pub iterations: usize,
    /// Deterministic content hash.
    pub content_hash: u64,
}

/// Perform economic dispatch using lambda iteration (bisection).
///
/// Finds the system lambda such that sum of generator outputs equals demand.
pub fn economic_dispatch(
    generators: &[Generator],
    demand_mw: f64,
    config: &DispatchConfig,
) -> DispatchResult {
    let n = generators.len();

    if n == 0 {
        return DispatchResult {
            outputs_mw: Vec::new(),
            total_generation_mw: 0.0,
            total_cost: 0.0,
            lambda: 0.0,
            converged: true,
            iterations: 0,
            content_hash: fnv1a(b"empty_dispatch"),
        };
    }

    // Check if demand is feasible
    let p_min_total: f64 = generators.iter().map(|g| g.p_min).sum();
    let p_max_total: f64 = generators.iter().map(|g| g.p_max).sum();

    if demand_mw < p_min_total {
        // All at minimum
        let outputs: Vec<f64> = generators.iter().map(|g| g.p_min).collect();
        let total_cost: f64 = generators
            .iter()
            .zip(outputs.iter())
            .map(|(g, &p)| g.cost(p))
            .sum();
        return make_result(outputs, p_min_total, total_cost, 0.0, true, 0);
    }

    if demand_mw > p_max_total {
        // All at maximum
        let outputs: Vec<f64> = generators.iter().map(|g| g.p_max).collect();
        let total_cost: f64 = generators
            .iter()
            .zip(outputs.iter())
            .map(|(g, &p)| g.cost(p))
            .sum();
        return make_result(outputs, p_max_total, total_cost, f64::INFINITY, true, 0);
    }

    // Lambda bisection bounds
    let mut lambda_lo = generators
        .iter()
        .map(|g| g.incremental_cost(g.p_min))
        .fold(f64::INFINITY, f64::min);
    let mut lambda_hi = generators
        .iter()
        .map(|g| g.incremental_cost(g.p_max))
        .fold(f64::NEG_INFINITY, f64::max);

    // Small guard to ensure bracket
    lambda_lo -= 1.0;
    lambda_hi += 1.0;

    let mut lambda = 0.5 * (lambda_lo + lambda_hi);
    let mut iterations = 0;
    let mut converged = false;

    for iter in 0..config.max_iterations {
        lambda = 0.5 * (lambda_lo + lambda_hi);

        let total_gen: f64 = generators.iter().map(|g| g.optimal_output(lambda)).sum();

        let mismatch = total_gen - demand_mw;

        iterations = iter + 1;

        if mismatch.abs() < config.tolerance_mw {
            converged = true;
            break;
        }

        if mismatch > 0.0 {
            // Too much generation → lower lambda
            lambda_hi = lambda;
        } else {
            // Not enough generation → raise lambda
            lambda_lo = lambda;
        }
    }

    let outputs: Vec<f64> = generators
        .iter()
        .map(|g| g.optimal_output(lambda))
        .collect();
    let total_gen: f64 = outputs.iter().sum();
    let total_cost: f64 = generators
        .iter()
        .zip(outputs.iter())
        .map(|(g, &p)| g.cost(p))
        .sum();

    make_result(
        outputs, total_gen, total_cost, lambda, converged, iterations,
    )
}

fn make_result(
    outputs: Vec<f64>,
    total_gen: f64,
    total_cost: f64,
    lambda: f64,
    converged: bool,
    iterations: usize,
) -> DispatchResult {
    let mut buf = Vec::with_capacity(outputs.len() * 8 + 16);
    buf.extend_from_slice(&lambda.to_bits().to_le_bytes());
    buf.extend_from_slice(&total_cost.to_bits().to_le_bytes());
    for &p in &outputs {
        buf.extend_from_slice(&p.to_bits().to_le_bytes());
    }
    let content_hash = fnv1a(&buf);

    DispatchResult {
        outputs_mw: outputs,
        total_generation_mw: total_gen,
        total_cost,
        lambda,
        converged,
        iterations,
        content_hash,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn two_generators() -> Vec<Generator> {
        vec![
            // Gen 1: C = 200 + 7P + 0.008P², P ∈ [50, 200]
            Generator::new(1, 200.0, 7.0, 0.008, 50.0, 200.0),
            // Gen 2: C = 180 + 6.3P + 0.009P², P ∈ [40, 150]
            Generator::new(2, 180.0, 6.3, 0.009, 40.0, 150.0),
        ]
    }

    #[test]
    fn empty_generators() {
        let result = economic_dispatch(&[], 100.0, &DispatchConfig::default());
        assert!(result.converged);
        assert!(result.outputs_mw.is_empty());
        assert_eq!(result.total_generation_mw, 0.0);
    }

    #[test]
    fn single_generator_exact_demand() {
        let gens = vec![Generator::new(1, 100.0, 5.0, 0.01, 0.0, 200.0)];
        let result = economic_dispatch(&gens, 100.0, &DispatchConfig::default());
        assert!(result.converged);
        assert!((result.outputs_mw[0] - 100.0).abs() < 0.1);
    }

    #[test]
    fn two_gen_equal_demand() {
        let gens = two_generators();
        let demand = 250.0; // within [90, 350]
        let result = economic_dispatch(&gens, demand, &DispatchConfig::default());
        assert!(result.converged);
        let total: f64 = result.outputs_mw.iter().sum();
        assert!(
            (total - demand).abs() < 0.1,
            "Total {total} != demand {demand}"
        );
    }

    #[test]
    fn equal_incremental_cost() {
        let gens = two_generators();
        let demand = 200.0;
        let result = economic_dispatch(&gens, demand, &DispatchConfig::default());
        assert!(result.converged);

        // At optimality, incremental costs should be approximately equal
        // (unless a generator is at its limit)
        let ic1 = gens[0].incremental_cost(result.outputs_mw[0]);
        let ic2 = gens[1].incremental_cost(result.outputs_mw[1]);

        let p1_at_limit = (result.outputs_mw[0] - gens[0].p_min).abs() < 0.1
            || (result.outputs_mw[0] - gens[0].p_max).abs() < 0.1;
        let p2_at_limit = (result.outputs_mw[1] - gens[1].p_min).abs() < 0.1
            || (result.outputs_mw[1] - gens[1].p_max).abs() < 0.1;

        if !p1_at_limit && !p2_at_limit {
            assert!((ic1 - ic2).abs() < 0.1, "IC1={ic1}, IC2={ic2}");
        }
    }

    #[test]
    fn demand_below_minimum() {
        let gens = two_generators();
        let demand = 10.0; // Below p_min_total = 90
        let result = economic_dispatch(&gens, demand, &DispatchConfig::default());
        assert!(result.converged);
        // All generators at minimum
        assert!((result.outputs_mw[0] - 50.0).abs() < 0.01);
        assert!((result.outputs_mw[1] - 40.0).abs() < 0.01);
    }

    #[test]
    fn demand_above_maximum() {
        let gens = two_generators();
        let demand = 500.0; // Above p_max_total = 350
        let result = economic_dispatch(&gens, demand, &DispatchConfig::default());
        assert!(result.converged);
        assert!((result.outputs_mw[0] - 200.0).abs() < 0.01);
        assert!((result.outputs_mw[1] - 150.0).abs() < 0.01);
    }

    #[test]
    fn generator_cost_monotonic() {
        let g = Generator::new(1, 100.0, 5.0, 0.01, 0.0, 200.0);
        let c1 = g.cost(50.0);
        let c2 = g.cost(100.0);
        let c3 = g.cost(150.0);
        assert!(c1 < c2);
        assert!(c2 < c3);
    }

    #[test]
    fn incremental_cost_increasing() {
        let g = Generator::new(1, 100.0, 5.0, 0.01, 0.0, 200.0);
        let ic1 = g.incremental_cost(50.0);
        let ic2 = g.incremental_cost(100.0);
        assert!(ic2 > ic1);
    }

    #[test]
    fn optimal_output_clamping() {
        let g = Generator::new(1, 0.0, 5.0, 0.01, 50.0, 200.0);
        // Lambda too low → p_min
        assert!((g.optimal_output(0.0) - 50.0).abs() < 1e-10);
        // Lambda very high → p_max
        assert!((g.optimal_output(1000.0) - 200.0).abs() < 1e-10);
    }

    #[test]
    fn content_hash_deterministic() {
        let gens = two_generators();
        let cfg = DispatchConfig::default();
        let r1 = economic_dispatch(&gens, 200.0, &cfg);
        let r2 = economic_dispatch(&gens, 200.0, &cfg);
        assert_eq!(r1.content_hash, r2.content_hash);
        assert_ne!(r1.content_hash, 0);
    }

    #[test]
    fn three_generators() {
        let gens = vec![
            Generator::new(1, 200.0, 7.0, 0.008, 50.0, 200.0),
            Generator::new(2, 180.0, 6.3, 0.009, 40.0, 150.0),
            Generator::new(3, 140.0, 6.8, 0.007, 30.0, 180.0),
        ];
        let demand = 400.0; // within [120, 530]
        let result = economic_dispatch(&gens, demand, &DispatchConfig::default());
        assert!(result.converged);
        let total: f64 = result.outputs_mw.iter().sum();
        assert!((total - demand).abs() < 0.1);
    }

    #[test]
    fn linear_cost_generator_runs_at_max_or_min() {
        // cost_c = 0 means linear cost; optimal_output should pick p_max or p_min
        let g = Generator::new(1, 0.0, 5.0, 0.0, 10.0, 100.0);
        assert!((g.optimal_output(10.0) - 100.0).abs() < 1e-10);
        assert!((g.optimal_output(4.0) - 10.0).abs() < 1e-10);
        assert!((g.optimal_output(5.0) - 100.0).abs() < 1e-10);
    }

    #[test]
    fn cost_at_zero_output() {
        let g = Generator::new(1, 200.0, 7.0, 0.008, 0.0, 200.0);
        assert!((g.cost(0.0) - 200.0).abs() < 1e-10);
    }

    #[test]
    fn dispatch_exact_at_p_min_total() {
        let gens = two_generators();
        // p_min_total = 50 + 40 = 90
        let result = economic_dispatch(&gens, 90.0, &DispatchConfig::default());
        assert!(result.converged);
        let total: f64 = result.outputs_mw.iter().sum();
        assert!((total - 90.0).abs() < 0.1);
    }

    #[test]
    fn dispatch_exact_at_p_max_total() {
        let gens = two_generators();
        // p_max_total = 200 + 150 = 350
        let result = economic_dispatch(&gens, 350.0, &DispatchConfig::default());
        assert!(result.converged);
        let total: f64 = result.outputs_mw.iter().sum();
        assert!((total - 350.0).abs() < 0.1);
    }

    #[test]
    fn dispatch_total_cost_positive() {
        let gens = two_generators();
        let result = economic_dispatch(&gens, 200.0, &DispatchConfig::default());
        assert!(result.total_cost > 0.0);
    }
}
