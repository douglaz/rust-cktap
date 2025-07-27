# Pure Rust Implementation Plan: rust-cktap

## Executive Summary

Fork and rewrite `rust-cktap` as a **pure Rust implementation** eliminating all PC/SC dependencies. This will enable static musl binary compilation while simplifying the codebase significantly.

## Simplified Architecture

**Clean Design**: Direct USB CCID communication only
```
Application → rust-cktap → rusb (USB) → Smart Card Reader
```

**No More**:
- PC/SC dependencies
- Feature flags for transport selection
- Backward compatibility layers
- FFI bindings

## Implementation Plan

### Phase 1: Project Setup (3 days)

#### New Crate Structure
```
rust-cktap/
├── Cargo.toml
├── lib/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── transport.rs      # USB CCID transport
│   │   ├── ccid.rs           # CCID protocol
│   │   ├── discovery.rs      # USB device enumeration
│   │   ├── commands.rs       # Existing commands (unchanged)
│   │   ├── apdu.rs           # Existing APDU (unchanged)
│   │   ├── tap_signer.rs     # Existing (unchanged)
│   │   ├── sats_card.rs      # Existing (unchanged)
│   │   └── emulator.rs       # Existing (unchanged)
│   └── examples/
│       └── basic.rs
├── cli/
│   ├── Cargo.toml
│   └── src/main.rs
└── README.md
```

#### Clean Dependencies
```toml
[dependencies]
# Pure Rust USB communication
rusb = "0.9"

# Existing dependencies (unchanged)
ciborium = "0.2.0"
serde = "1"
serde_bytes = "0.11"
tokio = { version = "1.44", features = ["macros"] }
thiserror = "2.0"
bitcoin = { version = "0.32", features = ["rand-std"] }
log = "0.4"
```

### Phase 2: Core Implementation (1 week)

#### CCID Protocol Implementation
```rust
// lib/src/ccid.rs
#[repr(C, packed)]
pub struct CcidHeader {
    pub message_type: u8,
    pub length: u32,
    pub slot: u8,
    pub sequence: u8,
    pub reserved: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
pub enum CcidMessageType {
    PcToRdrIccPowerOn = 0x62,
    PcToRdrXfrBlock = 0x6F,
    RdrToPcDataBlock = 0x80,
    RdrToPcSlotStatus = 0x81,
}

pub struct CcidCommand {
    pub header: CcidHeader,
    pub data: Vec<u8>,
}

pub struct CcidResponse {
    pub header: CcidHeader,
    pub data: Vec<u8>,
}
```

#### USB Transport Implementation
```rust
// lib/src/transport.rs
use rusb::{Context, DeviceHandle, UsbContext};

pub struct UsbTransport {
    device: DeviceHandle<Context>,
    endpoint_out: u8,
    endpoint_in: u8,
    sequence: AtomicU8,
}

impl CkTransport for UsbTransport {
    async fn transmit_apdu(&self, apdu: Vec<u8>) -> Result<Vec<u8>, Error> {
        // 1. Wrap APDU in CCID XfrBlock command
        let ccid_cmd = CcidCommand::xfr_block(
            0, // slot
            self.next_sequence(),
            apdu
        );

        // 2. Send via USB bulk transfer
        self.send_ccid_command(ccid_cmd).await?;

        // 3. Read CCID response
        let ccid_resp = self.read_ccid_response().await?;

        // 4. Extract APDU from DataBlock response
        Ok(ccid_resp.data)
    }
}

impl UsbTransport {
    async fn send_ccid_command(&self, cmd: CcidCommand) -> Result<(), Error> {
        let message = cmd.to_bytes();
        let timeout = Duration::from_secs(5);
        
        self.device.write_bulk(self.endpoint_out, &message, timeout)
            .map_err(Error::from)
    }

    async fn read_ccid_response(&self) -> Result<CcidResponse, Error> {
        let mut buffer = vec![0u8; 1024];
        let timeout = Duration::from_secs(5);
        
        let len = self.device.read_bulk(self.endpoint_in, &mut buffer, timeout)?;
        CcidResponse::from_bytes(&buffer[..len])
    }

    fn next_sequence(&self) -> u8 {
        self.sequence.fetch_add(1, Ordering::Relaxed)
    }
}
```

#### Device Discovery
```rust
// lib/src/discovery.rs
pub async fn find_first_card() -> Result<CkTapCard<UsbTransport>, Error> {
    let context = Context::new()?;
    
    for device in context.devices()?.iter() {
        if let Ok(handle) = open_ccid_device(&device) {
            let transport = UsbTransport::new(handle)?;
            return transport.to_cktap().await;
        }
    }
    
    Err(Error::DeviceNotFound)
}

fn open_ccid_device(device: &rusb::Device<Context>) -> Result<DeviceHandle<Context>, Error> {
    let desc = device.device_descriptor()?;
    let config = device.active_config_descriptor()?;
    
    // Find CCID interface (Class 0x0B)
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            if descriptor.class_code() == 0x0B {
                let handle = device.open()?;
                handle.claim_interface(interface.number())?;
                return Ok(handle);
            }
        }
    }
    
    Err(Error::NotCcidDevice)
}
```

### Phase 3: Integration (3 days)

#### Update Error Types
```rust
// lib/src/apdu.rs - Remove PC/SC error, add USB errors
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("CiborDe: {0}")]
    CiborDe(String),
    #[error("CiborValue: {0}")]
    CiborValue(String),
    #[error("CkTap: {0:?}")]
    CkTap(CkTapError),
    #[error("USB: {0}")]
    Usb(#[from] rusb::Error),
    #[error("CCID: {0}")]
    Ccid(String),
    #[error("Device not found")]
    DeviceNotFound,
    #[error("Not a CCID device")]
    NotCcidDevice,
    
    #[cfg(feature = "emulator")]
    #[error("Emulator: {0}")]
    Emulator(String),
}
```

#### Update CLI
```rust
// cli/src/main.rs - Simplified to USB only
use rust_cktap::discovery;

#[tokio::main]
async fn main() -> Result<(), Error> {
    #[cfg(not(feature = "emulator"))]
    let mut card = discovery::find_first_card().await?;

    #[cfg(feature = "emulator")]
    let mut card = rust_cktap::emulator::find_emulator().await?;

    // ... rest unchanged
}
```

### Phase 4: Testing & Cleanup (3 days)

#### Remove Legacy Code
- Delete `lib/src/pcsc.rs`
- Remove PC/SC from all `Cargo.toml` files
- Update examples to use USB discovery
- Clean up feature flags

#### Update Documentation
```rust
// lib/src/lib.rs - Clean exports
pub mod apdu;
pub mod commands;
pub mod transport;
pub mod ccid;
pub mod discovery;
pub mod sats_card;
pub mod tap_signer;

#[cfg(feature = "emulator")]
pub mod emulator;

// Re-exports
pub use discovery::find_first_card;
pub use transport::UsbTransport;
// ... other exports
```

## Key Simplifications

### What We're Removing
1. ❌ All PC/SC related code (`pcsc.rs`, PC/SC errors, PC/SC dependencies)
2. ❌ Feature flags for transport selection
3. ❌ Backward compatibility layers
4. ❌ FFI bindings and external C library dependencies
5. ❌ Complex build configuration

### What We're Keeping
1. ✅ All existing card logic (`TapSigner`, `SatsCard`, `apdu`, `commands`)
2. ✅ Emulator for testing
3. ✅ CLI interface (just simplified discovery)
4. ✅ All cryptographic operations
5. ✅ Existing API structure (`CkTransport` trait)

## Implementation Timeline

**Total: 2 weeks**

- **Days 1-3**: Project setup, clean dependencies, CCID protocol basics
- **Days 4-7**: USB transport implementation, device discovery
- **Days 8-10**: Integration, testing with real devices
- **Days 11-14**: Cleanup, documentation, final testing

## Static Binary Compilation

After implementation, static musl compilation becomes trivial:

```bash
# No external dependencies to worry about!
cargo build --target x86_64-unknown-linux-musl --release

# Results in a truly static binary
ldd target/x86_64-unknown-linux-musl/release/cktap-cli
# not a dynamic executable
```

## Dependencies Overview

### Before (Complex)
```toml
pcsc = { version = "2", optional = true }
# + system libpcsclite
# + pcscd daemon
# + complex static linking
```

### After (Simple)  
```toml
rusb = "0.9"
# Pure Rust, compiles anywhere
```

## Testing Strategy

### Unit Tests
- CCID protocol serialization/deserialization
- USB mock testing
- Error handling

### Integration Tests  
- Real device testing with tapsigner/satscard
- Static binary verification
- Cross-platform testing

### Migration Verification
- All existing functionality works
- Performance is comparable or better
- Static compilation succeeds

## Success Criteria

1. ✅ **Static musl binaries compile successfully**
2. ✅ **All tapsigner/satscard operations work identically**  
3. ✅ **No external dependencies required**
4. ✅ **Simplified build process**
5. ✅ **Cleaner, more maintainable codebase**

## Risk Mitigation

**Primary Risk**: CCID implementation compatibility

**Mitigation**: 
- Use proven CCID message structures from `kard-rs`
- Test with multiple reader types
- Follow USB-IF CCID specification exactly

**Secondary Risk**: USB permissions/platform differences  

**Mitigation**:
- Document udev rules for Linux
- Test on all target platforms
- Provide clear setup instructions

## Conclusion

This clean implementation eliminates complexity while solving your core requirement: **static musl binary compilation**. By removing all backward compatibility concerns, we get:

- **Simpler codebase** (fewer files, dependencies, complexity)
- **Pure Rust ecosystem** (better maintenance, security, performance)
- **True portability** (single static binary deployment)
- **Future-proof foundation** (no legacy dependencies)

The 2-week timeline is aggressive but achievable given that we're keeping all the existing card logic and just replacing the transport layer.