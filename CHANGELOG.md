# Changelog

All notable changes to ALICE-Energy will be documented in this file.

## [Unreleased]

### Changed
- License: `AGPL-3.0-only` → `LicenseRef-Proprietary`; `LICENSE-Proprietary` holds the text
- CI: `ci/**` の branch への送信でも CI を走らせる
- preflight: `.github/preflight-exact` を置いた `scripts/preflight.sh` が生成器の出力 (見出しの `generator rev` の版) と byte で一致しなければ送る前の確認が通らない
- 設定 file の comment を中身の説明だけにし、.gitignore を project の build 出力だけにした
- TRADEMARK_NOTICE が指す licence の file を LICENSE-* の file にした

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
