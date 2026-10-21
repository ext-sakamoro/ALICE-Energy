# Changelog

All notable changes to ALICE-Energy will be documented in this file.

## [Unreleased]

### Changed
- **License: `AGPL-3.0-only` → `AGPL-3.0-only OR LicenseRef-Commercial` (dual-licensed、2026-09-27)** AGPL 側の条件は変更なし (既存 AGPL 利用者への影響ゼロ)、商用という選択肢が追加されただけ SPDX が AGPL 単独だと cargo-deny / FOSSA / SBOM に「商用オプションなし」と見えるため宣言を dual に 変更点: SPDX / `LICENSE` → `LICENSE-AGPL` / `LICENSE-COMMERCIAL.md` (商用トリガー 6 条件 = クローズド製品・商用 SaaS・エッジ / ファームウェア配布・plugin 再配布・プラットフォーム NDA・保証、社内利用は AGPL 側で無償と明記) / README の選択肢表 商用窓口は法人 `contact@extoria.co.jp`
- CI: `ci/**` の branch への送信でも CI を走らせる

## [0.1.0] - 2026-02-23

### Added
- `node` — `PowerNode`, `NodeKind` (Generator/Consumer/Storage/Renewable)
- `grid` — `PowerGrid` with supply/demand balance, `Transmission` lines
- `phase` — `PhaseCorrection`, `FrequencyEvent`, synchronization checks
- `battery` — `BatteryState`, `BatteryChemistry`, degradation prediction, time-to-replacement
- `load_flow` — `DcLoadFlow` (Gauss-Seidel) and `AcLoadFlow` (Newton-Raphson) solvers
- `dispatch` — `economic_dispatch` merit-order generator scheduling
- `renewable` — `SolarPanel`, `WindTurbine`, capacity factor calculations
- FNV-1a shared hash utility
- Zero external dependencies
- 126 unit tests + 1 doc-test

### Fixed
- Loop variables only used as indices → iterator style in gauss_solve (clippy)
