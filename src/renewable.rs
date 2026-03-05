//! Renewable energy models (solar, wind)
//!
//! Solar: irradiance-based output with efficiency and temperature derating.
//! Wind: cubic power law with cut-in, rated, and cut-out speeds.
//!
//! Author: Moroya Sakamoto

use crate::fnv1a;

/// Solar panel parameters.
#[derive(Debug, Clone, Copy)]
pub struct SolarPanel {
    /// Peak rated power (kW) at STC (1000 W/m² irradiance).
    pub rated_kw: f64,
    /// Panel area (m²).
    pub area_m2: f64,
    /// Conversion efficiency (0.0 to 1.0). Typical 0.15-0.22.
    pub efficiency: f64,
    /// Temperature coefficient (%/°C from 25°C). Typically -0.004.
    pub temp_coeff: f64,
}

impl SolarPanel {
    /// Create a new solar panel.
    #[must_use] 
    pub fn new(rated_kw: f64, area_m2: f64, efficiency: f64) -> Self {
        Self {
            rated_kw,
            area_m2,
            efficiency,
            temp_coeff: -0.004,
        }
    }
}

/// Wind turbine parameters.
#[derive(Debug, Clone, Copy)]
pub struct WindTurbine {
    /// Rated power (kW).
    pub rated_kw: f64,
    /// Rotor swept area (m²).
    pub swept_area_m2: f64,
    /// Cut-in wind speed (m/s). Typical 3-4.
    pub cut_in_speed: f64,
    /// Rated wind speed (m/s). Typical 12-15.
    pub rated_speed: f64,
    /// Cut-out wind speed (m/s). Typical 25.
    pub cut_out_speed: f64,
    /// Power coefficient Cp. Theoretical max (Betz limit) = 0.593.
    pub cp: f64,
}

impl WindTurbine {
    /// Create a new wind turbine.
    #[must_use] 
    pub fn new(rated_kw: f64, swept_area_m2: f64) -> Self {
        Self {
            rated_kw,
            swept_area_m2,
            cut_in_speed: 3.0,
            rated_speed: 12.0,
            cut_out_speed: 25.0,
            cp: 0.45,
        }
    }
}

/// Solar output result.
#[derive(Debug, Clone, Copy)]
pub struct SolarOutput {
    /// Electrical output (kW).
    pub power_kw: f64,
    /// Capacity factor (0.0 to 1.0).
    pub capacity_factor: f64,
    /// Temperature derating factor (0.0 to 1.0+).
    pub temp_derating: f64,
    /// Deterministic content hash.
    pub content_hash: u64,
}

/// Wind output result.
#[derive(Debug, Clone, Copy)]
pub struct WindOutput {
    /// Electrical output (kW).
    pub power_kw: f64,
    /// Capacity factor (0.0 to 1.0).
    pub capacity_factor: f64,
    /// Operating region.
    pub region: WindRegion,
    /// Deterministic content hash.
    pub content_hash: u64,
}

/// Wind turbine operating region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindRegion {
    /// Below cut-in speed — no output.
    BelowCutIn,
    /// Between cut-in and rated — cubic power law.
    Partial,
    /// At or above rated but below cut-out — full rated power.
    Rated,
    /// Above cut-out — shutdown for safety.
    AboveCutOut,
}

/// Compute solar panel output.
///
/// `irradiance_w_m2`: Global horizontal irradiance (W/m²).
/// `ambient_temp_c`: Ambient temperature (°C).
#[must_use] 
pub fn solar_output(panel: &SolarPanel, irradiance_w_m2: f64, ambient_temp_c: f64) -> SolarOutput {
    if irradiance_w_m2 <= 0.0 || panel.rated_kw <= 0.0 {
        let mut buf = [0u8; 16];
        buf[..8].copy_from_slice(&irradiance_w_m2.to_bits().to_le_bytes());
        buf[8..16].copy_from_slice(&ambient_temp_c.to_bits().to_le_bytes());
        return SolarOutput {
            power_kw: 0.0,
            capacity_factor: 0.0,
            temp_derating: 1.0,
            content_hash: fnv1a(&buf),
        };
    }

    // Temperature derating: 1 + coeff * (T_cell - 25)
    // Approximate cell temp: T_cell ≈ T_ambient + 0.03 * irradiance
    let t_cell = ambient_temp_c + 0.03 * irradiance_w_m2;
    let temp_derating = (1.0 + panel.temp_coeff * (t_cell - 25.0)).max(0.0);

    // Power: Area * Irradiance * Efficiency * Derating
    // Convert W to kW: * 0.001
    let raw_kw = panel.area_m2 * irradiance_w_m2 * panel.efficiency * 0.001;
    let power_kw = (raw_kw * temp_derating).min(panel.rated_kw);

    let rcp_rated = 1.0 / panel.rated_kw;
    let cf = power_kw * rcp_rated;

    let mut buf = [0u8; 24];
    buf[..8].copy_from_slice(&irradiance_w_m2.to_bits().to_le_bytes());
    buf[8..16].copy_from_slice(&ambient_temp_c.to_bits().to_le_bytes());
    buf[16..24].copy_from_slice(&power_kw.to_bits().to_le_bytes());

    SolarOutput {
        power_kw,
        capacity_factor: cf,
        temp_derating,
        content_hash: fnv1a(&buf),
    }
}

/// Compute wind turbine output.
///
/// `wind_speed_ms`: Wind speed at hub height (m/s).
/// `air_density_kg_m3`: Air density (kg/m³). Default ~1.225 at sea level.
#[must_use] 
pub fn wind_output(
    turbine: &WindTurbine,
    wind_speed_ms: f64,
    air_density_kg_m3: f64,
) -> WindOutput {
    if wind_speed_ms < 0.0 || turbine.rated_kw <= 0.0 {
        let hash_buf = [0u8; 8];
        return WindOutput {
            power_kw: 0.0,
            capacity_factor: 0.0,
            region: WindRegion::BelowCutIn,
            content_hash: fnv1a(&hash_buf),
        };
    }

    let (power_kw, region) = if wind_speed_ms < turbine.cut_in_speed {
        (0.0, WindRegion::BelowCutIn)
    } else if wind_speed_ms >= turbine.cut_out_speed {
        (0.0, WindRegion::AboveCutOut)
    } else if wind_speed_ms >= turbine.rated_speed {
        (turbine.rated_kw, WindRegion::Rated)
    } else {
        // Cubic power law: P = 0.5 * ρ * A * Cp * v³, capped at rated
        let v3 = wind_speed_ms * wind_speed_ms * wind_speed_ms;
        let p = 0.5 * air_density_kg_m3 * turbine.swept_area_m2 * turbine.cp * v3 * 0.001;
        (p.min(turbine.rated_kw), WindRegion::Partial)
    };

    let rcp_rated = 1.0 / turbine.rated_kw;
    let cf = power_kw * rcp_rated;

    let mut buf = [0u8; 16];
    buf[..8].copy_from_slice(&wind_speed_ms.to_bits().to_le_bytes());
    buf[8..16].copy_from_slice(&power_kw.to_bits().to_le_bytes());

    WindOutput {
        power_kw,
        capacity_factor: cf,
        region,
        content_hash: fnv1a(&buf),
    }
}

/// Compute capacity factor from a time series of outputs.
///
/// Returns average output / rated capacity.
#[must_use] 
pub fn capacity_factor(outputs_kw: &[f64], rated_kw: f64) -> f64 {
    if outputs_kw.is_empty() || rated_kw <= 0.0 {
        return 0.0;
    }
    let rcp_n = 1.0 / outputs_kw.len() as f64;
    let avg: f64 = outputs_kw.iter().sum::<f64>() * rcp_n;
    (avg / rated_kw).clamp(0.0, 1.0)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_panel() -> SolarPanel {
        // 10 kW rated, 50 m², 20% efficiency
        SolarPanel::new(10.0, 50.0, 0.20)
    }

    fn test_turbine() -> WindTurbine {
        // 2000 kW rated, 4000 m² swept area
        WindTurbine::new(2000.0, 4000.0)
    }

    #[test]
    fn solar_no_irradiance() {
        let panel = test_panel();
        let out = solar_output(&panel, 0.0, 25.0);
        assert_eq!(out.power_kw, 0.0);
        assert_eq!(out.capacity_factor, 0.0);
    }

    #[test]
    fn solar_stc_conditions() {
        let panel = test_panel();
        // 1000 W/m², 25°C → raw = 50 * 1000 * 0.20 * 0.001 = 10 kW
        // Temp derating at 25°C cell: T_cell = 25 + 30 = 55°C
        // derating = 1 + (-0.004)*(55-25) = 0.88
        let out = solar_output(&panel, 1000.0, 25.0);
        assert!(out.power_kw > 0.0);
        assert!(out.power_kw <= panel.rated_kw);
        assert!(out.temp_derating < 1.0); // hot cell derates
    }

    #[test]
    fn solar_low_temp_boost() {
        let panel = test_panel();
        // At very low temperature, derating > 1 is possible
        // T_cell = -20 + 0.03*200 = -14°C
        // derating = 1 + (-0.004)*(-14-25) = 1 + 0.156 = 1.156
        let out = solar_output(&panel, 200.0, -20.0);
        assert!(out.temp_derating > 1.0);
    }

    #[test]
    fn solar_capped_at_rated() {
        let panel = test_panel();
        // Very high irradiance should not exceed rated
        let out = solar_output(&panel, 2000.0, 0.0);
        assert!(out.power_kw <= panel.rated_kw);
    }

    #[test]
    fn wind_below_cut_in() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 2.0, 1.225);
        assert_eq!(out.power_kw, 0.0);
        assert_eq!(out.region, WindRegion::BelowCutIn);
    }

    #[test]
    fn wind_partial_output() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 8.0, 1.225);
        assert!(out.power_kw > 0.0);
        assert!(out.power_kw < turbine.rated_kw);
        assert_eq!(out.region, WindRegion::Partial);
    }

    #[test]
    fn wind_rated_output() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 15.0, 1.225);
        assert!((out.power_kw - turbine.rated_kw).abs() < 1e-10);
        assert_eq!(out.region, WindRegion::Rated);
    }

    #[test]
    fn wind_above_cut_out() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 30.0, 1.225);
        assert_eq!(out.power_kw, 0.0);
        assert_eq!(out.region, WindRegion::AboveCutOut);
    }

    #[test]
    fn wind_cubic_law() {
        let turbine = test_turbine();
        // P ∝ v³: doubling speed → 8x power
        let out1 = wind_output(&turbine, 4.0, 1.225);
        let out2 = wind_output(&turbine, 8.0, 1.225);
        if out1.power_kw > 0.0 && out2.power_kw < turbine.rated_kw {
            let ratio = out2.power_kw / out1.power_kw;
            assert!((ratio - 8.0).abs() < 0.5, "ratio = {}", ratio);
        }
    }

    #[test]
    fn capacity_factor_full() {
        let outputs = vec![100.0; 10];
        let cf = capacity_factor(&outputs, 100.0);
        assert!((cf - 1.0).abs() < 1e-10);
    }

    #[test]
    fn capacity_factor_half() {
        let outputs = vec![50.0; 10];
        let cf = capacity_factor(&outputs, 100.0);
        assert!((cf - 0.5).abs() < 1e-10);
    }

    #[test]
    fn capacity_factor_empty() {
        let cf = capacity_factor(&[], 100.0);
        assert_eq!(cf, 0.0);
    }

    #[test]
    fn solar_content_hash_deterministic() {
        let panel = test_panel();
        let o1 = solar_output(&panel, 800.0, 30.0);
        let o2 = solar_output(&panel, 800.0, 30.0);
        assert_eq!(o1.content_hash, o2.content_hash);
        assert_ne!(o1.content_hash, 0);
    }

    #[test]
    fn wind_content_hash_deterministic() {
        let turbine = test_turbine();
        let o1 = wind_output(&turbine, 10.0, 1.225);
        let o2 = wind_output(&turbine, 10.0, 1.225);
        assert_eq!(o1.content_hash, o2.content_hash);
        assert_ne!(o1.content_hash, 0);
    }

    #[test]
    fn solar_negative_irradiance_returns_zero() {
        let panel = test_panel();
        let out = solar_output(&panel, -100.0, 25.0);
        assert_eq!(out.power_kw, 0.0);
    }

    #[test]
    fn solar_zero_rated_panel_returns_zero() {
        let panel = SolarPanel::new(0.0, 50.0, 0.20);
        let out = solar_output(&panel, 1000.0, 25.0);
        assert_eq!(out.power_kw, 0.0);
    }

    #[test]
    fn wind_negative_speed_returns_zero() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, -5.0, 1.225);
        assert_eq!(out.power_kw, 0.0);
        assert_eq!(out.region, WindRegion::BelowCutIn);
    }

    #[test]
    fn wind_zero_rated_turbine_returns_zero() {
        let turbine = WindTurbine::new(0.0, 4000.0);
        let out = wind_output(&turbine, 10.0, 1.225);
        assert_eq!(out.power_kw, 0.0);
    }

    #[test]
    fn wind_at_exact_cut_in_speed() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 3.0, 1.225);
        // At exactly cut_in, we are NOT below cut_in, so partial output
        assert!(out.power_kw >= 0.0);
        assert_eq!(out.region, WindRegion::Partial);
    }

    #[test]
    fn wind_at_exact_rated_speed() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 12.0, 1.225);
        assert!((out.power_kw - turbine.rated_kw).abs() < 1e-10);
        assert_eq!(out.region, WindRegion::Rated);
    }

    #[test]
    fn wind_at_exact_cut_out_speed() {
        let turbine = test_turbine();
        let out = wind_output(&turbine, 25.0, 1.225);
        assert_eq!(out.power_kw, 0.0);
        assert_eq!(out.region, WindRegion::AboveCutOut);
    }

    #[test]
    fn capacity_factor_zero_rated() {
        assert_eq!(capacity_factor(&[50.0, 60.0], 0.0), 0.0);
    }

    #[test]
    fn capacity_factor_clamps_above_one() {
        // Outputs exceed rated — should clamp to 1.0
        let cf = capacity_factor(&[200.0, 200.0], 100.0);
        assert!((cf - 1.0).abs() < 1e-10);
    }
}
