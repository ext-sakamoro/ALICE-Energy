//! FACTS コントローラ
//!
//! Flexible AC Transmission Systems: STATCOM, SVC, UPFC。
//! 無効電力補償と電圧安定化。

/// FACTS デバイス種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactsType {
    /// Static Synchronous Compensator。
    Statcom,
    /// Static Var Compensator。
    Svc,
    /// Unified Power Flow Controller。
    Upfc,
}

impl core::fmt::Display for FactsType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Statcom => write!(f, "STATCOM"),
            Self::Svc => write!(f, "SVC"),
            Self::Upfc => write!(f, "UPFC"),
        }
    }
}

/// STATCOM パラメータ。
#[derive(Debug, Clone)]
pub struct StatcomParams {
    /// 定格容量 (`MVAr`)。
    pub rating_mvar: f64,
    /// 目標電圧 (p.u.)。
    pub voltage_setpoint: f64,
    /// ドループ係数。
    pub droop: f64,
}

/// STATCOM 状態。
#[derive(Debug, Clone)]
pub struct Statcom {
    /// パラメータ。
    pub params: StatcomParams,
    /// 現在の無効電力出力 (`MVAr`)。
    pub q_output: f64,
    /// バス ID。
    pub bus_id: u32,
}

impl Statcom {
    /// 新しい STATCOM を作成。
    #[must_use]
    pub const fn new(bus_id: u32, params: StatcomParams) -> Self {
        Self {
            params,
            q_output: 0.0,
            bus_id,
        }
    }

    /// 電圧偏差に基づく無効電力出力を計算。
    ///
    /// `Q = (V_ref - V_actual) / droop` (定格容量でクランプ)。
    pub fn compute_output(&mut self, actual_voltage: f64) {
        let deviation = self.params.voltage_setpoint - actual_voltage;
        let q = deviation / self.params.droop;
        self.q_output = q.clamp(-self.params.rating_mvar, self.params.rating_mvar);
    }
}

/// SVC パラメータ。
#[derive(Debug, Clone)]
pub struct SvcParams {
    /// 誘導性容量 (MVAr、吸収)。
    pub inductive_mvar: f64,
    /// 容量性容量 (MVAr、供給)。
    pub capacitive_mvar: f64,
    /// 目標電圧 (p.u.)。
    pub voltage_setpoint: f64,
    /// スロープ (ドループ)。
    pub slope: f64,
}

/// SVC 状態。
#[derive(Debug, Clone)]
pub struct Svc {
    /// パラメータ。
    pub params: SvcParams,
    /// 現在のサセプタンス (p.u.)。
    pub susceptance: f64,
    /// バス ID。
    pub bus_id: u32,
}

impl Svc {
    /// 新しい SVC を作成。
    #[must_use]
    pub const fn new(bus_id: u32, params: SvcParams) -> Self {
        Self {
            params,
            susceptance: 0.0,
            bus_id,
        }
    }

    /// 電圧偏差に基づくサセプタンスを計算。
    pub fn compute_susceptance(&mut self, actual_voltage: f64) {
        let deviation = self.params.voltage_setpoint - actual_voltage;
        let b = deviation / self.params.slope;
        // 容量性 (正) / 誘導性 (負) でクランプ
        self.susceptance = b.clamp(-self.params.inductive_mvar, self.params.capacitive_mvar);
    }

    /// 無効電力出力 (`MVAr`)。
    #[must_use]
    pub fn q_output(&self, voltage: f64) -> f64 {
        self.susceptance * voltage * voltage
    }
}

/// UPFC パラメータ。
#[derive(Debug, Clone)]
pub struct UpfcParams {
    /// シリーズ側定格 (MVA)。
    pub series_rating_mva: f64,
    /// シャント側定格 (`MVAr`)。
    pub shunt_rating_mvar: f64,
    /// 目標潮流 (MW)。
    pub power_setpoint_mw: f64,
    /// 目標電圧 (p.u.)。
    pub voltage_setpoint: f64,
}

/// UPFC 状態。
#[derive(Debug, Clone)]
pub struct Upfc {
    /// パラメータ。
    pub params: UpfcParams,
    /// シリーズ注入有効電力 (MW)。
    pub p_series: f64,
    /// シリーズ注入無効電力 (`MVAr`)。
    pub q_series: f64,
    /// シャント無効電力 (`MVAr`)。
    pub q_shunt: f64,
    /// 送電側バス ID。
    pub from_bus: u32,
    /// 受電側バス ID。
    pub to_bus: u32,
}

impl Upfc {
    /// 新しい UPFC を作成。
    #[must_use]
    pub const fn new(from_bus: u32, to_bus: u32, params: UpfcParams) -> Self {
        Self {
            params,
            p_series: 0.0,
            q_series: 0.0,
            q_shunt: 0.0,
            from_bus,
            to_bus,
        }
    }

    /// 潮流制御を計算。
    pub fn compute_control(&mut self, actual_power_mw: f64, actual_voltage: f64) {
        // シリーズ側: 潮流偏差補正
        let p_error = self.params.power_setpoint_mw - actual_power_mw;
        self.p_series = p_error.clamp(
            -self.params.series_rating_mva,
            self.params.series_rating_mva,
        );

        // シャント側: 電圧維持
        let v_error = self.params.voltage_setpoint - actual_voltage;
        self.q_shunt = (v_error * 100.0).clamp(
            -self.params.shunt_rating_mvar,
            self.params.shunt_rating_mvar,
        );
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_type_display() {
        assert_eq!(FactsType::Statcom.to_string(), "STATCOM");
        assert_eq!(FactsType::Svc.to_string(), "SVC");
        assert_eq!(FactsType::Upfc.to_string(), "UPFC");
    }

    #[test]
    fn statcom_voltage_high() {
        let mut s = Statcom::new(
            1,
            StatcomParams {
                rating_mvar: 100.0,
                voltage_setpoint: 1.0,
                droop: 0.05,
            },
        );
        s.compute_output(1.05); // 電圧高い → 吸収
        assert!(s.q_output < 0.0);
    }

    #[test]
    fn statcom_voltage_low() {
        let mut s = Statcom::new(
            1,
            StatcomParams {
                rating_mvar: 100.0,
                voltage_setpoint: 1.0,
                droop: 0.05,
            },
        );
        s.compute_output(0.95); // 電圧低い → 供給
        assert!(s.q_output > 0.0);
    }

    #[test]
    fn statcom_clamp() {
        let mut s = Statcom::new(
            1,
            StatcomParams {
                rating_mvar: 10.0,
                voltage_setpoint: 1.0,
                droop: 0.001,
            },
        );
        s.compute_output(0.5); // 大きな偏差
        assert!(s.q_output <= 10.0);
    }

    #[test]
    fn svc_susceptance() {
        let mut svc = Svc::new(
            1,
            SvcParams {
                inductive_mvar: 50.0,
                capacitive_mvar: 100.0,
                voltage_setpoint: 1.0,
                slope: 0.02,
            },
        );
        svc.compute_susceptance(0.98);
        assert!(svc.susceptance > 0.0);
    }

    #[test]
    fn svc_q_output() {
        let svc = Svc {
            params: SvcParams {
                inductive_mvar: 50.0,
                capacitive_mvar: 100.0,
                voltage_setpoint: 1.0,
                slope: 0.02,
            },
            susceptance: 0.5,
            bus_id: 1,
        };
        let q = svc.q_output(1.0);
        assert!((q - 0.5).abs() < 0.01);
    }

    #[test]
    fn svc_clamp() {
        let mut svc = Svc::new(
            1,
            SvcParams {
                inductive_mvar: 10.0,
                capacitive_mvar: 20.0,
                voltage_setpoint: 1.0,
                slope: 0.001,
            },
        );
        svc.compute_susceptance(0.0);
        assert!(svc.susceptance <= 20.0);
    }

    #[test]
    fn upfc_power_control() {
        let mut upfc = Upfc::new(
            1,
            2,
            UpfcParams {
                series_rating_mva: 50.0,
                shunt_rating_mvar: 30.0,
                power_setpoint_mw: 100.0,
                voltage_setpoint: 1.0,
            },
        );
        upfc.compute_control(80.0, 0.98);
        assert!(upfc.p_series > 0.0); // 潮流不足 → 注入
        assert!(upfc.q_shunt > 0.0); // 電圧低い → 供給
    }

    #[test]
    fn upfc_clamp() {
        let mut upfc = Upfc::new(
            1,
            2,
            UpfcParams {
                series_rating_mva: 10.0,
                shunt_rating_mvar: 5.0,
                power_setpoint_mw: 1000.0,
                voltage_setpoint: 1.0,
            },
        );
        upfc.compute_control(0.0, 0.0);
        assert!(upfc.p_series <= 10.0);
        assert!(upfc.q_shunt <= 5.0);
    }

    #[test]
    fn upfc_buses() {
        let upfc = Upfc::new(
            5,
            10,
            UpfcParams {
                series_rating_mva: 50.0,
                shunt_rating_mvar: 30.0,
                power_setpoint_mw: 100.0,
                voltage_setpoint: 1.0,
            },
        );
        assert_eq!(upfc.from_bus, 5);
        assert_eq!(upfc.to_bus, 10);
    }

    #[test]
    fn statcom_at_setpoint() {
        let mut s = Statcom::new(
            1,
            StatcomParams {
                rating_mvar: 100.0,
                voltage_setpoint: 1.0,
                droop: 0.05,
            },
        );
        s.compute_output(1.0);
        assert!((s.q_output).abs() < 0.01);
    }

    #[test]
    fn facts_type_eq() {
        assert_eq!(FactsType::Statcom, FactsType::Statcom);
        assert_ne!(FactsType::Statcom, FactsType::Svc);
    }

    #[test]
    fn upfc_voltage_at_setpoint() {
        let mut upfc = Upfc::new(
            1,
            2,
            UpfcParams {
                series_rating_mva: 50.0,
                shunt_rating_mvar: 30.0,
                power_setpoint_mw: 100.0,
                voltage_setpoint: 1.0,
            },
        );
        upfc.compute_control(100.0, 1.0);
        assert!((upfc.q_shunt).abs() < 0.01);
        assert!((upfc.p_series).abs() < 0.01);
    }
}
