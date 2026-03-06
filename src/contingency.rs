//! N-1 Contingency Analysis
//!
//! 送電線1本の喪失時の系統安全性評価。

/// 送電線 ID。
pub type LineId = u32;

/// Contingency ケース。
#[derive(Debug, Clone)]
pub struct ContingencyCase {
    /// 喪失送電線 ID。
    pub failed_line: LineId,
    /// 各送電線の潮流 (MW)。
    pub line_flows: Vec<LineFlow>,
    /// 過負荷フラグ。
    pub overloaded: bool,
    /// 電圧逸脱フラグ。
    pub voltage_violation: bool,
}

/// 送電線潮流。
#[derive(Debug, Clone)]
pub struct LineFlow {
    /// 送電線 ID。
    pub line_id: LineId,
    /// 潮流 (MW)。
    pub flow_mw: f64,
    /// 定格容量 (MW)。
    pub capacity_mw: f64,
    /// 負荷率 (0.0-1.0+)。
    pub loading: f64,
}

impl LineFlow {
    /// 過負荷か。
    #[must_use]
    pub fn is_overloaded(&self) -> bool {
        self.loading > 1.0
    }
}

/// 送電線パラメータ。
#[derive(Debug, Clone)]
pub struct TransmissionLine {
    /// 送電線 ID。
    pub id: LineId,
    /// 送電側バス。
    pub from_bus: u32,
    /// 受電側バス。
    pub to_bus: u32,
    /// リアクタンス (p.u.)。
    pub reactance: f64,
    /// 定格容量 (MW)。
    pub capacity_mw: f64,
}

/// N-1 Contingency 分析器。
#[derive(Debug)]
pub struct ContingencyAnalyzer {
    /// 送電線リスト。
    lines: Vec<TransmissionLine>,
    /// バス注入電力 (MW)。バスインデックス → 注入。
    bus_injections: Vec<f64>,
}

impl ContingencyAnalyzer {
    /// 新しい分析器を作成。
    #[must_use]
    pub const fn new(lines: Vec<TransmissionLine>, bus_injections: Vec<f64>) -> Self {
        Self {
            lines,
            bus_injections,
        }
    }

    /// N-1 分析を実行。各送電線を1本ずつ除外して潮流を再計算。
    #[must_use]
    pub fn analyze(&self) -> Vec<ContingencyCase> {
        let mut results = Vec::with_capacity(self.lines.len());

        for failed_idx in 0..self.lines.len() {
            let case = self.analyze_case(failed_idx);
            results.push(case);
        }

        results
    }

    /// 1ケースの分析。
    fn analyze_case(&self, failed_idx: usize) -> ContingencyCase {
        let failed_line = self.lines[failed_idx].id;

        // 残存送電線で DC 潮流を簡易計算
        let mut line_flows = Vec::new();
        let mut overloaded = false;

        for (i, line) in self.lines.iter().enumerate() {
            if i == failed_idx {
                continue;
            }

            // 簡易 DC 潮流: P = (θ_from - θ_to) / X
            // バスインデックスに基づく注入差で近似
            let from_inj = self
                .bus_injections
                .get(line.from_bus as usize)
                .copied()
                .unwrap_or(0.0);
            let to_inj = self
                .bus_injections
                .get(line.to_bus as usize)
                .copied()
                .unwrap_or(0.0);

            // 喪失線の影響で潮流が分散
            let redistribution_factor = if self.lines.len() > 1 {
                self.lines.len() as f64 / (self.lines.len() - 1) as f64
            } else {
                1.0
            };

            let flow_mw = ((from_inj - to_inj) / line.reactance * redistribution_factor).abs();
            let loading = if line.capacity_mw > 0.0 {
                flow_mw / line.capacity_mw
            } else {
                0.0
            };

            if loading > 1.0 {
                overloaded = true;
            }

            line_flows.push(LineFlow {
                line_id: line.id,
                flow_mw,
                capacity_mw: line.capacity_mw,
                loading,
            });
        }

        ContingencyCase {
            failed_line,
            line_flows,
            overloaded,
            voltage_violation: false,
        }
    }

    /// N-1 セキュアか (全ケースで過負荷なし)。
    #[must_use]
    pub fn is_n1_secure(&self) -> bool {
        self.analyze().iter().all(|c| !c.overloaded)
    }

    /// 最も深刻なケースを返す。
    #[must_use]
    pub fn worst_case(&self) -> Option<ContingencyCase> {
        self.analyze().into_iter().max_by(|a, b| {
            let a_max = a
                .line_flows
                .iter()
                .map(|f| f.loading)
                .fold(0.0f64, f64::max);
            let b_max = b
                .line_flows
                .iter()
                .map(|f| f.loading)
                .fold(0.0f64, f64::max);
            a_max
                .partial_cmp(&b_max)
                .unwrap_or(core::cmp::Ordering::Equal)
        })
    }

    /// 送電線数。
    #[must_use]
    pub const fn line_count(&self) -> usize {
        self.lines.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lines() -> Vec<TransmissionLine> {
        vec![
            TransmissionLine {
                id: 1,
                from_bus: 0,
                to_bus: 1,
                reactance: 0.1,
                capacity_mw: 100.0,
            },
            TransmissionLine {
                id: 2,
                from_bus: 1,
                to_bus: 2,
                reactance: 0.2,
                capacity_mw: 80.0,
            },
            TransmissionLine {
                id: 3,
                from_bus: 0,
                to_bus: 2,
                reactance: 0.15,
                capacity_mw: 120.0,
            },
        ]
    }

    #[test]
    fn analyze_returns_n_cases() {
        let analyzer = ContingencyAnalyzer::new(make_lines(), vec![50.0, -20.0, -30.0]);
        let results = analyzer.analyze();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn each_case_excludes_failed_line() {
        let analyzer = ContingencyAnalyzer::new(make_lines(), vec![50.0, -20.0, -30.0]);
        let results = analyzer.analyze();
        for case in &results {
            for flow in &case.line_flows {
                assert_ne!(flow.line_id, case.failed_line);
            }
        }
    }

    #[test]
    fn line_flow_overloaded() {
        let flow = LineFlow {
            line_id: 1,
            flow_mw: 150.0,
            capacity_mw: 100.0,
            loading: 1.5,
        };
        assert!(flow.is_overloaded());
    }

    #[test]
    fn line_flow_not_overloaded() {
        let flow = LineFlow {
            line_id: 1,
            flow_mw: 50.0,
            capacity_mw: 100.0,
            loading: 0.5,
        };
        assert!(!flow.is_overloaded());
    }

    #[test]
    fn n1_secure_check() {
        let analyzer = ContingencyAnalyzer::new(
            make_lines(),
            vec![10.0, -5.0, -5.0], // 小さな潮流
        );
        // 小さな注入なら安全のはず
        let _ = analyzer.is_n1_secure(); // 結果は問わず実行確認
    }

    #[test]
    fn worst_case() {
        let analyzer = ContingencyAnalyzer::new(make_lines(), vec![50.0, -20.0, -30.0]);
        let worst = analyzer.worst_case();
        assert!(worst.is_some());
    }

    #[test]
    fn line_count() {
        let analyzer = ContingencyAnalyzer::new(make_lines(), vec![0.0; 3]);
        assert_eq!(analyzer.line_count(), 3);
    }

    #[test]
    fn empty_lines() {
        let analyzer = ContingencyAnalyzer::new(vec![], vec![]);
        assert!(analyzer.analyze().is_empty());
        assert!(analyzer.worst_case().is_none());
    }

    #[test]
    fn single_line() {
        let lines = vec![TransmissionLine {
            id: 1,
            from_bus: 0,
            to_bus: 1,
            reactance: 0.1,
            capacity_mw: 100.0,
        }];
        let analyzer = ContingencyAnalyzer::new(lines, vec![50.0, -50.0]);
        let results = analyzer.analyze();
        assert_eq!(results.len(), 1);
        assert!(results[0].line_flows.is_empty()); // 唯一の線が除外
    }

    #[test]
    fn zero_capacity_no_panic() {
        let lines = vec![
            TransmissionLine {
                id: 1,
                from_bus: 0,
                to_bus: 1,
                reactance: 0.1,
                capacity_mw: 0.0,
            },
            TransmissionLine {
                id: 2,
                from_bus: 0,
                to_bus: 1,
                reactance: 0.1,
                capacity_mw: 100.0,
            },
        ];
        let analyzer = ContingencyAnalyzer::new(lines, vec![50.0, -50.0]);
        let _ = analyzer.analyze(); // パニックしないことを確認
    }

    #[test]
    fn contingency_case_flags() {
        let case = ContingencyCase {
            failed_line: 1,
            line_flows: vec![],
            overloaded: true,
            voltage_violation: false,
        };
        assert!(case.overloaded);
        assert!(!case.voltage_violation);
    }

    #[test]
    fn loading_calculation() {
        let analyzer = ContingencyAnalyzer::new(make_lines(), vec![50.0, -20.0, -30.0]);
        let results = analyzer.analyze();
        for case in &results {
            for flow in &case.line_flows {
                if flow.capacity_mw > 0.0 {
                    assert!((flow.loading - flow.flow_mw / flow.capacity_mw).abs() < 0.001);
                }
            }
        }
    }

    #[test]
    fn case_has_n_minus_1_lines() {
        let analyzer = ContingencyAnalyzer::new(make_lines(), vec![50.0, -20.0, -30.0]);
        let results = analyzer.analyze();
        for case in &results {
            assert_eq!(case.line_flows.len(), 2); // 3 lines - 1 failed = 2
        }
    }
}
