# cktap-direct

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/notmandatory/rust-cktap/blob/master/LICENSE)
[![CI](https://github.com/notmandatory/rust-cktap/actions/workflows/test.yml/badge.svg)](https://github.com/notmandatory/rust-cktap/actions/workflows/test.yml)
[![rustc](https://img.shields.io/badge/rustc-1.88.0%2B-lightgrey.svg)](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/)

A Rust implementation of the [Coinkite Tap Protocol](https://github.com/coinkite/coinkite-tap-proto) (cktap)
for use with [SATSCARD], [TAPSIGNER], and [SATSCHIP] products.

## Fork Overview

`cktap-direct` is a fork of [rust-cktap](https://github.com/notmandatory/rust-cktap) that replaces the PC/SC dependency with a direct USB CCID implementation. The main differences from upstream are:

- **Direct USB Access**: Uses `rusb` (libusb wrapper) instead of PC/SC middleware
- **Static Binary Support**: Can compile to fully static musl binaries
- **Native CCID Protocol**: Implements the USB CCID protocol directly
- **Enhanced CLI**: Structured commands with JSON output by default for better scripting

### Original Project

This project provides APDU message encoding and decoding, cvc authentication, certificate chain verification, and card response verification.

It is up to the crate user to send and receive the raw cktap APDU messages via NFC to the card by implementing the `CkTransport` trait. This fork provides a USB CCID transport implementation that works with USB smart card readers. Mobile users are expected to implement `CkTransport` using the iOS or Android provided libraries.

### Supported Features

- [x] [IOS Applet Select](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#first-step-iso-applet-select)
- [x] [CVC Authentication](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#authenticating-commands-with-cvc)

#### Shared Commands

- [x] [status](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#status)
- [x] [read](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#status) (messages)
  - [x] response verification
- [x] [derive](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#derive) (messages)
  - [x] response verification
- [x] [certs](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#certs)
- [x] [new](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#new)
- [x] [nfc](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#nfc)
- [x] [sign](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#sign) (messages)
  - [ ] response verification
- [x] [wait](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#wait)

#### SATSCARD-Only Commands

- [x] [unseal](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#unseal)
- [x] [dump](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#dump)

#### TAPSIGNER-Only Commands

- [x] [change](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#change)
- [x] [xpub](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#xpub)
- [x] [backup](https://github.com/coinkite/coinkite-tap-proto/blob/master/docs/protocol.md#backup)

### Automated Testing with Emulator

1. Install and start [cktap emulator](https://github.com/coinkite/coinkite-tap-proto/blob/master/emulator/README.md)
   - TapSigner: `./ecard.py emulate -t --no-init`
   - SatsCard: `./ecard.py emulate -s`
2. run tests: `cargo test --features emulator`

### Manual Testing with real cards

#### Prerequisites

1. USB PCSC NFC card reader, for example:
   - [OMNIKEY 5022 CL](https://www.hidglobal.com/products/omnikey-5022-reader)
2. Coinkite SATSCARD, TAPSIGNER, or SATSCHIP cards
   Install vendor PCSC driver
3. Connect NFC reader to desktop system
4. Place SATSCARD, TAPSIGNER, or SATSCHIP on reader

#### Run CLI

The CLI has been restructured with subcommands for different card types:

```bash
# Show help
cargo run --bin cktap-direct -- --help

# Auto-detect card type commands
cargo run --bin cktap-direct -- auto status
cargo run --bin cktap-direct -- auto certs

# SatsCard-specific commands
cargo run --bin cktap-direct -- satscard status
cargo run --bin cktap-direct -- satscard address
cargo run --bin cktap-direct -- satscard read
cargo run --bin cktap-direct -- satscard derive

# TapSigner-specific commands (requires CVC/PIN)
CKTAP_CVC=123456 cargo run --bin cktap-direct -- tapsigner status
CKTAP_CVC=123456 cargo run --bin cktap-direct -- tapsigner read
CKTAP_CVC=123456 cargo run --bin cktap-direct -- tapsigner derive --path 84,0,0
CKTAP_CVC=123456 cargo run --bin cktap-direct -- tapsigner sign "message to sign"

# Output format (JSON by default)
cargo run --bin cktap-direct -- --format json auto status
cargo run --bin cktap-direct -- --format plain auto status  # Note: plain format not fully implemented
```

**Note**: The CLI now outputs JSON by default for easy scripting and integration. Use `--format plain` for human-readable output (currently shows "not implemented" for most commands).

## Building

This project defaults to building static musl binaries for maximum portability:

```bash
# Build debug binary (static musl)
cargo build

# Build release binary (static musl)
cargo build --release

# The binary will be at: target/x86_64-unknown-linux-musl/release/cktap-direct
```

If you need a dynamically linked binary:

```bash
# Build for your host platform
cargo build --target x86_64-unknown-linux-gnu
```

## Minimum Supported Rust Version (MSRV)

This library should always compile with any valid combination of features on Rust **1.88.0**.



[SATSCARD]: https://satscard.com/
[TAPSIGNER]: https://tapsigner.com/
[SATSCHIP]: https://satschip.com/
