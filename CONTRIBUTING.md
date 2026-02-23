# Contributing to ALICE-Energy

## Build

```bash
cargo build
```

## Test

```bash
cargo test
```

## Lint

```bash
cargo clippy -- -W clippy::all
cargo fmt -- --check
cargo doc --no-deps 2>&1 | grep warning
```

## Design Constraints

- **Deterministic simulation**: same input always produces same grid state (integer timestamps, FNV-1a hashing).
- **DC and AC load flow**: DC uses Gauss-Seidel iteration; AC uses Newton-Raphson with Jacobian and Gaussian elimination.
- **Battery degradation**: cycle-based and calendar aging models for multiple chemistries.
- **Phase synchronization**: frequency event detection and correction within configurable tolerances.
- **Economic dispatch**: merit-order scheduling respecting generator min/max limits.
- **Zero external dependencies**: all physics, math, and grid logic is self-contained.
