set shell := ["bash", "-euo", "pipefail", "-c"]

# List available tasks.
default:
    @just --list

# Format the workspace.
fmt:
    cargo fmt --all

# Check formatting without changing files.
fmt-check:
    cargo fmt --all --check

# Run clippy with the same settings as CI.
lint:
    cargo clippy --locked --workspace --exclude stm32wb55-embassy --all-targets -- -D warnings

# Run formatting and lint checks.
check: fmt-check lint

# Test every host-side workspace crate.
test:
    cargo test --locked --workspace --exclude stm32wb55-embassy

# Test stm32wb-hci against every declared firmware and stack profile.
test-firmwares:
    #!/usr/bin/env bash
    set -euo pipefail
    firmwares="$(cargo run --locked -q -p stm32wb-compliance -- list-supported)"
    profiles="$(cargo run --locked -q -p stm32wb-compliance -- list-profiles)"
    test -n "$firmwares"
    test -n "$profiles"
    for firmware in $firmwares; do
        for profile in $profiles; do
            if ! cargo test --locked -p stm32wb-hci --lib --tests --no-default-features --features="$firmware,$profile"; then
                printf 'test failed: %s,%s\n' "$firmware" "$profile" >&2
                exit 1
            fi
        done
    done

# Check the Embassy example for its embedded target.
example:
    cargo check --locked -p stm32wb55-embassy --target thumbv7em-none-eabi

# Run all local checks and tests.
test-all: check test test-firmwares example

# Build release binaries for every firmware feature, with and without defmt.
build-firmwares:
    #!/usr/bin/env bash
    set -euo pipefail
    firmwares="$(cargo run --locked -q -p stm32wb-compliance -- list-supported)"
    test -n "$firmwares"
    for firmware in $firmwares; do
        cargo build --locked -p stm32wb-hci --no-default-features --features="$firmware,stack-full-extended" --release --target=thumbv7em-none-eabihf
        cargo build --locked -p stm32wb-hci --no-default-features --features="$firmware,stack-full-extended,defmt" --release --target=thumbv7em-none-eabihf
    done

# Compare every supported target with a local STM32CubeWB checkout.
compliance:
    cargo run --locked -p stm32wb-compliance -- check --all-supported --deny
