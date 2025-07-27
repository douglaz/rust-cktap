# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is `cktap-direct`, a Rust library and CLI tool for communicating directly with Coinkite TapSigner and SatsCard devices via USB CCID protocol. This is a fork of `rust-cktap` that replaces PC/SC middleware dependencies with pure Rust USB communication, enabling static musl binary compilation.

## Development Commands

### Build & Run
```bash
# Build the project
nix develop -c cargo build

# Build release
nix develop -c cargo build --release

# Build static musl binary
nix develop -c cargo build --release --target x86_64-unknown-linux-musl

# Run CLI
nix develop -c cargo run --bin cktap-direct -- [ARGS]

# Test with TapSigner
CKTAP_CVC=123456 nix develop -c cargo run --bin cktap-direct -- derive --path 84,0,0,0,0
```

### Testing & Quality
```bash
# Run tests
nix develop -c cargo test

# Format code
nix develop -c cargo fmt --all

# Check formatting
nix develop -c cargo fmt --all -- --config format_code_in_doc_comments=true --check

# Run clippy
nix develop -c cargo clippy --all-features --all-targets -- -D warnings

# Test specific configurations
nix develop -c cargo test --no-default-features
nix develop -c cargo test --features default
```

## Git Workflow

### Branch Naming Convention

Use descriptive branch names following this pattern:
- `feature/description` - New features
- `fix/description` - Bug fixes 
- `chore/description` - Maintenance tasks
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring

**Examples:**
- `feature/add-satscard-unseal`
- `fix/usb-timeout-handling`
- `chore/upgrade-rust-edition-2024`
- `docs/update-api-examples`

### Development Workflow

1. **Create a new branch for each change:**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes and test locally:**
   ```bash
   # Always test before committing
   nix develop -c cargo test
   nix develop -c cargo clippy --all-features --all-targets -- -D warnings
   nix develop -c cargo fmt --all -- --config format_code_in_doc_comments=true --check
   ```

3. **Commit with descriptive messages:**
   ```bash
   git add .
   git commit -m "feat: add support for new feature
   
   - Implement specific functionality
   - Add tests for edge cases
   - Update documentation"
   ```

4. **Push and create PR:**
   ```bash
   git push origin feature/your-feature-name
   # Create PR via GitHub CLI or web interface
   nix develop -c gh pr create --title "Add new feature" --body "Description of changes"
   ```

### Commit Message Format

Use conventional commit format:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style changes (formatting, etc.)
- `refactor:` - Code refactoring
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks
- `ci:` - CI/CD changes

### Quality Requirements

**Before any commit:**
1. Code must format correctly: `cargo fmt --all`
2. Code must pass clippy: `cargo clippy --all-features --all-targets -- -D warnings`
3. All tests must pass: `cargo test`
4. Static musl build must work: `cargo build --release --target x86_64-unknown-linux-musl`

**Never commit:**
- Backup files (*.backup, *.bak)
- Temporary files
- IDE-specific files
- Proposal or documentation drafts not intended for the repo

## Architecture

### Module Structure
- `lib/src/` - Core library code
  - `apdu.rs` - APDU command/response definitions
  - `ccid.rs` - CCID protocol implementation
  - `usb_transport.rs` - USB communication layer
  - `discovery.rs` - Device discovery and enumeration
  - `commands.rs` - High-level command trait
  - `sats_card.rs` - SatsCard-specific functionality
  - `tap_signer.rs` - TapSigner-specific functionality
- `cli/src/main.rs` - CLI application
- `cktap-ffi/` - FFI bindings for other languages

### Key Patterns

1. **Error Handling**: Uses `thiserror` for structured errors and `anyhow` for CLI
2. **Async**: Built on Tokio with async/await throughout
3. **USB Communication**: Direct CCID protocol over USB bulk transfers
4. **Transport Abstraction**: `CkTransport` trait allows pluggable backends

### Environment Variables
- `CKTAP_CVC` - Card Verification Code for testing
- `RUST_LOG` - Control logging level (default: info)

## Development Environment

### Using Nix
The project uses Nix flakes for reproducible development environments:

```bash
# Enter development shell with all dependencies
nix develop

# Run commands in the shell
nix develop -c cargo build
nix develop -c cargo test
```

The Nix environment provides:
- Rust toolchain with musl target
- Static libusb and libudev libraries
- pkg-config and build tools
- GitHub CLI (gh)

### Dependencies
- **Core**: Rust with edition 2024
- **USB**: rusb (libusb wrapper)
- **Crypto**: bitcoin crate with secp256k1
- **Serialization**: ciborium (CBOR), serde
- **CLI**: clap for argument parsing

## Testing

### Hardware Testing
- Requires physical TapSigner or SatsCard device
- Test with various card readers (OMNIKEY preferred)
- Verify static binary deployment

### Unit Tests
```bash
# Run all tests
nix develop -c cargo test

# Test specific modules
nix develop -c cargo test ccid::
nix develop -c cargo test usb_transport::
nix develop -c cargo test discovery::
```

### Integration Testing
```bash
# Test with real hardware (requires device)
CKTAP_CVC=123456 nix develop -c cargo run --bin cktap-direct -- debug
```

## Deployment

### Static Binary
```bash
# Build portable static binary
nix develop -c cargo build --release --target x86_64-unknown-linux-musl

# Verify it's static
ldd target/x86_64-unknown-linux-musl/release/cktap-direct
```

## Troubleshooting

### Common Issues
1. **USB permissions**: Add udev rules or run with elevated privileges
2. **Card reader compatibility**: OMNIKEY readers work best
3. **Static linking**: Use Nix environment for proper library configuration

### Debug Commands
```bash
# List USB devices
lsusb

# Check for CCID readers
nix develop -c cargo run --example usb_test

# Enable debug logging
RUST_LOG=debug nix develop -c cargo run --bin cktap-direct -- debug
```