//! Battery degradation prediction.

/// Unique battery identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatteryId(pub u64);

/// Battery chemistry types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryChemistry {
    LithiumIon,
    LithiumIronPhosphate,
    SolidState,
    SodiumIon,
    FlowBattery,
}

/// State of a battery unit.
#[derive(Debug, Clone)]
pub struct BatteryState {
    pub id: BatteryId,
    pub chemistry: BatteryChemistry,
    pub capacity_kwh: f64,
    pub state_of_charge: f64,
    pub cycle_count: u32,
    pub temperature_c: f64,
    pub internal_resistance_ohm: f64,
    pub max_cycles: u32,
}

impl BatteryState {
    pub fn new(id: u64, chemistry: BatteryChemistry, capacity_kwh: f64, max_cycles: u32) -> Self {
        Self {
            id: BatteryId(id),
            chemistry,
            capacity_kwh,
            state_of_charge: 1.0,
            cycle_count: 0,
            temperature_c: 25.0,
            internal_resistance_ohm: 0.01,
            max_cycles,
        }
    }

    /// Health as percentage (0-100).
    #[inline]
    pub fn health_percentage(&self) -> f64 {
        if self.max_cycles == 0 { return 0.0; }
        (100.0 * (1.0 - self.cycle_count as f64 / self.max_cycles as f64)).clamp(0.0, 100.0)
    }

    /// Remaining usable capacity in kWh.
    #[inline]
    pub fn remaining_capacity_kwh(&self) -> f64 {
        self.capacity_kwh * (self.health_percentage() / 100.0) * self.state_of_charge
    }

    /// Charge the battery by `kwh`, clamping SoC to 1.0.
    pub fn charge(&mut self, kwh: f64) {
        let effective_capacity = self.capacity_kwh * (self.health_percentage() / 100.0);
        if effective_capacity <= 0.0 { return; }
        self.state_of_charge = (self.state_of_charge + kwh / effective_capacity).min(1.0);
    }

    /// Discharge by `kwh`, returns actual kWh discharged.
    pub fn discharge(&mut self, kwh: f64) -> f64 {
        let effective_capacity = self.capacity_kwh * (self.health_percentage() / 100.0);
        if effective_capacity <= 0.0 { return 0.0; }
        let available = self.state_of_charge * effective_capacity;
        let actual = kwh.min(available);
        self.state_of_charge = (self.state_of_charge - actual / effective_capacity).max(0.0);
        actual
    }

    /// Record a complete charge/discharge cycle.
    pub fn complete_cycle(&mut self) {
        self.cycle_count += 1;
    }
}

/// Predict health percentage after additional cycles.
pub fn predict_degradation(battery: &BatteryState, additional_cycles: u32) -> f64 {
    if battery.max_cycles == 0 { return 0.0; }
    let total = battery.cycle_count + additional_cycles;
    (100.0 * (1.0 - total as f64 / battery.max_cycles as f64)).clamp(0.0, 100.0)
}

/// Days until health drops below threshold.
pub fn time_to_replacement(battery: &BatteryState, cycles_per_day: f64, min_health_pct: f64) -> f64 {
    if cycles_per_day <= 0.0 || battery.max_cycles == 0 { return f64::INFINITY; }
    let current_health = battery.health_percentage();
    if current_health <= min_health_pct { return 0.0; }

    // health = 100 * (1 - (cycle_count + cpd*days) / max_cycles)
    // min_health = 100 * (1 - (cycle_count + cpd*days) / max_cycles)
    // days = (max_cycles * (1 - min_health/100) - cycle_count) / cpd
    let cycles_remaining = battery.max_cycles as f64 * (1.0 - min_health_pct / 100.0) - battery.cycle_count as f64;
    if cycles_remaining <= 0.0 { return 0.0; }
    cycles_remaining / cycles_per_day
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_new_battery() {
        let b = BatteryState::new(1, BatteryChemistry::LithiumIon, 100.0, 1000);
        assert!((b.health_percentage() - 100.0).abs() < 1e-10);
    }

    #[test]
    fn health_after_cycles() {
        let mut b = BatteryState::new(1, BatteryChemistry::LithiumIon, 100.0, 1000);
        for _ in 0..500 { b.complete_cycle(); }
        assert!((b.health_percentage() - 50.0).abs() < 1e-10);
    }

    #[test]
    fn charge_and_discharge() {
        let mut b = BatteryState::new(1, BatteryChemistry::LithiumIronPhosphate, 100.0, 3000);
        b.state_of_charge = 0.5;
        b.charge(25.0);
        assert!(b.state_of_charge > 0.5);
        let discharged = b.discharge(10.0);
        assert!(discharged > 0.0);
    }

    #[test]
    fn discharge_returns_available() {
        let mut b = BatteryState::new(1, BatteryChemistry::SolidState, 10.0, 5000);
        b.state_of_charge = 0.1; // 1 kWh available
        let actual = b.discharge(100.0); // request 100, only ~1 available
        assert!(actual < 2.0);
        assert!(b.state_of_charge < 0.01);
    }

    #[test]
    fn degradation_prediction() {
        let b = BatteryState::new(1, BatteryChemistry::LithiumIon, 100.0, 1000);
        let health = predict_degradation(&b, 500);
        assert!((health - 50.0).abs() < 1e-10);
    }

    #[test]
    fn time_to_replacement_calculation() {
        let b = BatteryState::new(1, BatteryChemistry::LithiumIon, 100.0, 1000);
        // min_health = 20% → need 800 cycles → at 2/day = 400 days
        let days = time_to_replacement(&b, 2.0, 20.0);
        assert!((days - 400.0).abs() < 1e-10);
    }

    #[test]
    fn chemistry_variants() {
        let chemistries = [
            BatteryChemistry::LithiumIon,
            BatteryChemistry::LithiumIronPhosphate,
            BatteryChemistry::SolidState,
            BatteryChemistry::SodiumIon,
            BatteryChemistry::FlowBattery,
        ];
        for c in &chemistries {
            let b = BatteryState::new(1, *c, 50.0, 2000);
            assert_eq!(b.chemistry, *c);
        }
    }
}
