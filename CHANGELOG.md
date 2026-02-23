# Changelog

All notable changes to ALICE-Energy will be documented in this file.

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
