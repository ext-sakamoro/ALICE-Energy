//! 安定性マージン評価
//!
//! 過渡安定性と小信号安定性の評価。

/// 発電機パラメータ (古典モデル)。
#[derive(Debug, Clone)]
pub struct GeneratorDynamic {
    /// 発電機 ID。
    pub id: u32,
    /// 慣性定数 H (秒)。
    pub inertia_h: f64,
    /// 過渡リアクタンス Xd' (p.u.)。
    pub xd_prime: f64,
    /// 制動係数 D。
    pub damping_d: f64,
    /// 機械的入力 (p.u.)。
    pub pm: f64,
    /// 内部電圧 (p.u.)。
    pub eq_prime: f64,
}

/// 過渡安定性シミュレーション結果。
#[derive(Debug, Clone)]
pub struct TransientResult {
    /// 時間ステップ (秒)。
    pub time_steps: Vec<f64>,
    /// ロータ角度 (rad)。
    pub rotor_angles: Vec<f64>,
    /// 角速度偏差 (rad/s)。
    pub speed_deviations: Vec<f64>,
    /// 安定か。
    pub is_stable: bool,
    /// 最大ロータ角度 (rad)。
    pub max_angle: f64,
    /// クリティカルクリアリングタイム (秒、推定)。
    pub critical_clearing_time: f64,
}

/// 小信号安定性の固有値。
#[derive(Debug, Clone)]
pub struct Eigenvalue {
    /// 実部。
    pub real: f64,
    /// 虚部。
    pub imag: f64,
}

impl Eigenvalue {
    /// 安定か (実部 < 0)。
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.real < 0.0
    }

    /// 減衰比。
    #[must_use]
    pub fn damping_ratio(&self) -> f64 {
        let mag = self.real.hypot(self.imag);
        if mag == 0.0 {
            return 0.0;
        }
        -self.real / mag
    }

    /// 振動周波数 (Hz)。
    #[must_use]
    pub fn frequency_hz(&self) -> f64 {
        self.imag.abs() / (2.0 * core::f64::consts::PI)
    }
}

/// 小信号安定性結果。
#[derive(Debug, Clone)]
pub struct SmallSignalResult {
    /// 固有値。
    pub eigenvalues: Vec<Eigenvalue>,
    /// 全固有値が安定か。
    pub is_stable: bool,
    /// 最小減衰比。
    pub min_damping_ratio: f64,
}

/// 過渡安定性シミュレーション (スイング方程式)。
///
/// 単機無限バス (SMIB) モデル。
#[must_use]
pub fn simulate_transient(
    gen: &GeneratorDynamic,
    fault_duration: f64,
    sim_duration: f64,
    dt: f64,
) -> TransientResult {
    let omega_s = 2.0 * core::f64::consts::PI * 50.0; // 50Hz 系統
    let steps = (sim_duration / dt) as usize;
    let fault_steps = (fault_duration / dt) as usize;

    let mut delta: f64 = (gen.pm / gen.eq_prime).asin(); // 初期角度
    let mut omega: f64 = 0.0; // 角速度偏差

    let mut time_steps = Vec::with_capacity(steps);
    let mut rotor_angles = Vec::with_capacity(steps);
    let mut speed_deviations = Vec::with_capacity(steps);
    let mut max_angle: f64 = delta.abs();

    for i in 0..steps {
        let t = i as f64 * dt;
        time_steps.push(t);
        rotor_angles.push(delta);
        speed_deviations.push(omega);

        // 電気出力 (故障中は 0)
        let pe = if i < fault_steps {
            0.0
        } else {
            gen.eq_prime * delta.sin() / gen.xd_prime
        };

        // スイング方程式: 2H/ωs * dω/dt = Pm - Pe - D*ω
        let accel = gen.damping_d.mul_add(-omega, gen.pm - pe) * omega_s / (2.0 * gen.inertia_h);
        omega += accel * dt;
        delta += omega * dt;

        max_angle = max_angle.max(delta.abs());

        // 不安定判定 (180度超過)
        if delta.abs() > core::f64::consts::PI {
            return TransientResult {
                time_steps,
                rotor_angles,
                speed_deviations,
                is_stable: false,
                max_angle,
                critical_clearing_time: fault_duration,
            };
        }
    }

    TransientResult {
        time_steps,
        rotor_angles,
        speed_deviations,
        is_stable: true,
        max_angle,
        critical_clearing_time: fault_duration,
    }
}

/// 小信号安定性解析 (線形化スイング方程式)。
///
/// A 行列の固有値を計算 (2x2 SMIB モデル)。
#[must_use]
pub fn analyze_small_signal(gen: &GeneratorDynamic) -> SmallSignalResult {
    let omega_s = 2.0 * core::f64::consts::PI * 50.0;
    let delta_0 = (gen.pm / gen.eq_prime).asin();

    // 同期化係数 Ks = Eq' * cos(δ0) / Xd'
    let ks = gen.eq_prime * delta_0.cos() / gen.xd_prime;

    // A 行列 (2x2):
    // [0, 1]
    // [-Ks*ωs/(2H), -D*ωs/(2H)]
    let a22 = -gen.damping_d * omega_s / (2.0 * gen.inertia_h);
    let a21 = -ks * omega_s / (2.0 * gen.inertia_h);

    // 固有値: λ = (a22 ± sqrt(a22² + 4*a21)) / 2
    let discriminant = a22 * a22 + 4.0 * a21;

    let eigenvalues = if discriminant >= 0.0 {
        let sqrt_d = discriminant.sqrt();
        vec![
            Eigenvalue {
                real: f64::midpoint(a22, sqrt_d),
                imag: 0.0,
            },
            Eigenvalue {
                real: (a22 - sqrt_d) / 2.0,
                imag: 0.0,
            },
        ]
    } else {
        let sqrt_d = (-discriminant).sqrt();
        vec![
            Eigenvalue {
                real: a22 / 2.0,
                imag: sqrt_d / 2.0,
            },
            Eigenvalue {
                real: a22 / 2.0,
                imag: -sqrt_d / 2.0,
            },
        ]
    };

    let is_stable = eigenvalues.iter().all(Eigenvalue::is_stable);
    let min_damping = eigenvalues
        .iter()
        .map(Eigenvalue::damping_ratio)
        .fold(f64::INFINITY, f64::min);

    SmallSignalResult {
        eigenvalues,
        is_stable,
        min_damping_ratio: min_damping,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn default_gen() -> GeneratorDynamic {
        GeneratorDynamic {
            id: 1,
            inertia_h: 5.0,
            xd_prime: 0.3,
            damping_d: 2.0,
            pm: 0.8,
            eq_prime: 1.1,
        }
    }

    #[test]
    fn transient_stable() {
        let gen = default_gen();
        let result = simulate_transient(&gen, 0.05, 2.0, 0.001);
        assert!(result.is_stable);
    }

    #[test]
    fn transient_unstable_long_fault() {
        // 低慣性・低制動で長い故障 → 不安定
        let gen = GeneratorDynamic {
            inertia_h: 1.0,
            damping_d: 0.1,
            ..default_gen()
        };
        let result = simulate_transient(&gen, 2.0, 5.0, 0.001);
        assert!(!result.is_stable);
    }

    #[test]
    fn transient_max_angle() {
        let gen = default_gen();
        let result = simulate_transient(&gen, 0.05, 2.0, 0.001);
        assert!(result.max_angle > 0.0);
    }

    #[test]
    fn transient_time_steps() {
        let gen = default_gen();
        let result = simulate_transient(&gen, 0.05, 1.0, 0.01);
        assert_eq!(result.time_steps.len(), 100);
    }

    #[test]
    fn small_signal_stable() {
        let gen = default_gen();
        let result = analyze_small_signal(&gen);
        assert!(result.is_stable);
    }

    #[test]
    fn small_signal_eigenvalues() {
        let gen = default_gen();
        let result = analyze_small_signal(&gen);
        assert_eq!(result.eigenvalues.len(), 2);
    }

    #[test]
    fn eigenvalue_damping_ratio() {
        let ev = Eigenvalue {
            real: -1.0,
            imag: 2.0,
        };
        assert!(ev.is_stable());
        assert!(ev.damping_ratio() > 0.0);
    }

    #[test]
    fn eigenvalue_frequency() {
        let ev = Eigenvalue {
            real: -1.0,
            imag: 6.283,
        };
        assert!((ev.frequency_hz() - 1.0).abs() < 0.01);
    }

    #[test]
    fn eigenvalue_unstable() {
        let ev = Eigenvalue {
            real: 0.5,
            imag: 1.0,
        };
        assert!(!ev.is_stable());
    }

    #[test]
    fn eigenvalue_zero() {
        let ev = Eigenvalue {
            real: 0.0,
            imag: 0.0,
        };
        assert_eq!(ev.damping_ratio(), 0.0);
        assert_eq!(ev.frequency_hz(), 0.0);
    }

    #[test]
    fn small_signal_min_damping() {
        let gen = default_gen();
        let result = analyze_small_signal(&gen);
        assert!(result.min_damping_ratio >= 0.0);
    }

    #[test]
    fn small_signal_undamped() {
        let gen = GeneratorDynamic {
            damping_d: 0.0,
            ..default_gen()
        };
        let result = analyze_small_signal(&gen);
        // ゼロ制動 → 減衰比 = 0
        assert!((result.min_damping_ratio).abs() < 0.01);
    }

    #[test]
    fn transient_rotor_angles_populated() {
        let gen = default_gen();
        let result = simulate_transient(&gen, 0.05, 0.5, 0.01);
        assert!(!result.rotor_angles.is_empty());
        assert_eq!(result.rotor_angles.len(), result.speed_deviations.len());
    }
}
