# Pure Rust Implementation Proposal: rust-cktap

## Executive Summary

This document outlines a proposal to fork and rewrite the `rust-cktap` library to eliminate PC/SC dependencies and enable true static musl binary compilation. The implementation will use pure Rust libraries (`rusb` for USB communication and custom CCID protocol implementation) while maintaining full compatibility with the existing API.

## Problem Statement

### Current Limitations

1. **Static Musl Compilation Issues**:
   - PC/SC requires `libpcsclite.so` dynamic library
   - Complex static linking with Alpine Linux package conflicts
   - Runtime dependency on `pcscd` daemon

2. **External Dependencies**:
   - FFI bindings to system libraries
   - Platform-specific PC/SC implementations
   - Complex build requirements for cross-compilation

3. **Deployment Challenges**:
   - Cannot create truly portable static binaries
   - Requires PC/SC service installation and configuration
   - Platform-specific driver requirements

## Proposed Solution

### Architecture Overview

Replace PC/SC layer with direct USB CCID communication:

```
Current:  Application → rust-cktap → pcsc-rust → libpcsclite → pcscd → USB CCID
Proposed: Application → rust-cktap → rusb-ccid → USB CCID (direct)
```

### Key Benefits

1. **True Static Binaries**: No external C library dependencies
2. **Portable Deployment**: Single binary works across platforms
3. **Simplified Build**: Pure Rust compilation chain
4. **Better Control**: Direct hardware communication
5. **Reduced Attack Surface**: Fewer system dependencies

## Technical Design

### 1. Core Architecture

#### Transport Abstraction (Unchanged)
```rust
pub trait CkTransport: Sized {
    fn transmit_apdu(&self, command_apdu: Vec<u8>) -> impl Future<Output = Result<Vec<u8>, Error>>;
    // ... existing methods
}
```

#### New USB CCID Transport
```rust
pub struct UsbCcidTransport {
    device: rusb::DeviceHandle<rusb::Context>,
    endpoint_out: u8,
    endpoint_in: u8,
    sequence: AtomicU8,
    timeout: Duration,
}

impl CkTransport for UsbCcidTransport {
    async fn transmit_apdu(&self, apdu: Vec<u8>) -> Result<Vec<u8>, Error> {
        // CCID protocol implementation
    }
}
```

### 2. CCID Protocol Implementation

#### Message Structure
```rust
#[repr(C, packed)]
pub struct CcidHeader {
    pub message_type: u8,
    pub length: u32,      // LE format
    pub slot: u8,
    pub sequence: u8,
    pub message_specific: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
pub enum MessageType {
    // PC to RDR commands
    PcToRdrIccPowerOn = 0x62,
    PcToRdrIccPowerOff = 0x63,
    PcToRdrGetSlotStatus = 0x65,
    PcToRdrXfrBlock = 0x6F,
    
    // RDR to PC responses
    RdrToPcDataBlock = 0x80,
    RdrToPcSlotStatus = 0x81,
    RdrToPcParameters = 0x82,
}
```

#### CCID Communication Flow
```rust
impl UsbCcidTransport {
    async fn send_ccid_command(&self, cmd: CcidCommand) -> Result<CcidResponse, Error> {
        // 1. Build CCID message
        let message = self.build_ccid_message(cmd)?;
        
        // 2. Send via USB bulk transfer
        self.device.write_bulk(self.endpoint_out, &message, self.timeout)?;
        
        // 3. Read response
        let mut buffer = vec![0u8; 1024];
        let len = self.device.read_bulk(self.endpoint_in, &mut buffer, self.timeout)?;
        
        // 4. Parse CCID response
        self.parse_ccid_response(&buffer[..len])
    }
    
    async fn power_on_card(&self, slot: u8) -> Result<Vec<u8>, Error> {
        let cmd = CcidCommand::IccPowerOn { slot, voltage: VoltageSelection::Automatic };
        let resp = self.send_ccid_command(cmd).await?;
        
        match resp {
            CcidResponse::DataBlock { data, .. } => Ok(data), // ATR
            CcidResponse::SlotStatus { error, .. } => Err(Error::CcidError(error)),
        }
    }
}
```

### 3. Device Discovery

#### USB Device Enumeration
```rust
pub async fn find_ccid_devices() -> Result<Vec<UsbCcidTransport>, Error> {
    let context = rusb::Context::new()?;
    let mut devices = Vec::new();
    
    for device in context.devices()?.iter() {
        if is_ccid_device(&device)? {
            let handle = device.open()?;
            let transport = UsbCcidTransport::new(handle)?;
            devices.push(transport);
        }
    }
    
    Ok(devices)
}

fn is_ccid_device(device: &rusb::Device<rusb::Context>) -> Result<bool, Error> {
    let desc = device.device_descriptor()?;
    let config = device.active_config_descriptor()?;
    
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            // USB Class 0x0B = Smart Card (CCID)
            if descriptor.class_code() == 0x0B {
                return Ok(true);
            }
        }
    }
    
    Ok(false)
}
```

### 4. Error Handling

#### Comprehensive Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ... existing errors
    
    #[error("USB error: {0}")]
    Usb(#[from] rusb::Error),
    
    #[error("CCID error: {0:?}")]
    Ccid(CcidError),
    
    #[error("Device not found")]
    DeviceNotFound,
    
    #[error("Invalid CCID response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone, Copy)]
pub enum CcidError {
    CommandAborted = 0xFF,
    IccMute = 0xFE,
    XfrParityError = 0xFD,
    XfrOverrun = 0xFC,
    // ... other CCID error codes
}
```

## Implementation Plan

### Phase 1: Foundation (Week 1)
- [ ] Create new crate structure with feature flags
- [ ] Implement basic CCID protocol structures
- [ ] Add `rusb` dependency and basic USB enumeration
- [ ] Create `UsbCcidTransport` skeleton

### Phase 2: Core CCID Implementation (Week 2)
- [ ] Implement CCID message serialization/deserialization
- [ ] Add USB bulk transfer communication
- [ ] Implement basic CCID commands (PowerOn, GetSlotStatus, XfrBlock)
- [ ] Add error handling and timeout management

### Phase 3: Integration (Week 3)
- [ ] Integrate with existing `CkTransport` trait
- [ ] Implement device discovery and connection
- [ ] Add comprehensive error mapping
- [ ] Create feature flags for transport selection

### Phase 4: Testing & Validation (Week 4)
- [ ] Test with real tapsigner/satscard devices
- [ ] Validate against existing test suite
- [ ] Performance comparison with PC/SC implementation
- [ ] Cross-platform testing (Linux, macOS, Windows)

### Phase 5: Documentation & Release (Week 5)
- [ ] Update documentation and examples
- [ ] Migration guide from PC/SC version
- [ ] Static binary compilation guide
- [ ] Release preparation

## File Structure

```
rust-cktap/
├── Cargo.toml                 # Updated with feature flags
├── lib/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs             # Feature-gated exports
│   │   ├── ccid/              # New CCID implementation
│   │   │   ├── mod.rs
│   │   │   ├── protocol.rs    # CCID protocol structures
│   │   │   ├── transport.rs   # USB CCID transport
│   │   │   ├── commands.rs    # CCID command implementations
│   │   │   └── errors.rs      # CCID-specific errors
│   │   ├── usb.rs             # USB device discovery
│   │   ├── pcsc.rs            # Legacy PC/SC transport (optional)
│   │   └── ... (existing files)
│   └── examples/
│       ├── usb_ccid.rs        # USB CCID example
│       └── pcsc.rs            # Legacy PC/SC example
├── cli/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs            # Updated for transport selection
└── README.md                  # Updated documentation
```

## Dependencies

### New Dependencies
```toml
[dependencies]
# Pure Rust USB communication
rusb = "0.9"

# Async runtime (already present)
tokio = { version = "1.44", features = ["macros"] }

# Serialization (already present)
ciborium = "0.2.0"
serde = "1"

# Error handling (already present)
thiserror = "2.0"

# Bitcoin/crypto (already present)
bitcoin = { version = "0.32", features = ["rand-std"] }

# Optional PC/SC (for backward compatibility)
pcsc = { version = "2", optional = true }
```

### Feature Flags
```toml
[features]
default = ["usb-ccid"]
usb-ccid = ["rusb"]
pcsc = ["dep:pcsc"]
both = ["usb-ccid", "pcsc"]
```

## Migration Strategy

### Backward Compatibility
1. **Feature Flags**: Maintain PC/SC support as optional feature
2. **API Compatibility**: Keep existing public API unchanged
3. **Transport Selection**: Runtime or compile-time transport selection
4. **Gradual Migration**: Allow users to test USB implementation while keeping PC/SC fallback

### Migration Path
```rust
// Automatic transport selection
pub async fn find_first_card() -> Result<CkTapCard<impl CkTransport>, Error> {
    #[cfg(feature = "usb-ccid")]
    {
        if let Ok(card) = crate::usb::find_first().await {
            return Ok(card);
        }
    }
    
    #[cfg(feature = "pcsc")]
    {
        return crate::pcsc::find_first().await;
    }
    
    #[cfg(not(any(feature = "usb-ccid", feature = "pcsc")))]
    {
        compile_error!("At least one transport feature must be enabled");
    }
}
```

## Testing Strategy

### Unit Tests
- CCID protocol message serialization/deserialization
- USB device enumeration mocking
- Error handling and edge cases
- Transport trait compliance

### Integration Tests
- Real device communication tests
- Cross-platform compatibility
- Performance benchmarks
- Static binary compilation verification

### Test Devices
- Coinkite Tapsigner
- Coinkite Satscard
- Generic CCID smart card readers (for compatibility)

## Security Considerations

### Attack Surface Reduction
- Eliminate PC/SC daemon dependency
- Reduce external library attack surface
- Direct hardware communication reduces intermediary risks

### USB Security
- Input validation for all USB responses
- Timeout handling to prevent DoS
- Proper error handling for malformed CCID responses

### Cryptographic Operations
- Maintain existing cryptographic implementations
- No changes to signing or authentication logic
- Preserve security properties of original implementation

## Performance Considerations

### Expected Performance
- **Latency**: Slightly lower (direct USB vs PC/SC overhead)
- **Throughput**: Comparable to PC/SC implementation
- **Memory**: Lower (no PC/SC service overhead)
- **CPU**: Comparable (same cryptographic operations)

### Optimizations
- Connection pooling for multiple operations
- Bulk transfer optimization
- Async/await for non-blocking I/O

## Risk Assessment

### High Risk
- **Hardware Compatibility**: Some CCID readers may have quirks
- **Platform Differences**: USB behavior variations across OS
- **Testing Coverage**: Limited access to all device types

### Medium Risk
- **Performance Regression**: Unlikely but possible
- **Complex Debugging**: Lower-level USB debugging required
- **Migration Complexity**: Users need to update build processes

### Low Risk
- **API Breaking Changes**: Maintained through trait abstraction
- **Security Regression**: Same protocol, different transport
- **Maintenance Burden**: Pure Rust is easier to maintain

### Mitigation Strategies
1. **Extensive Testing**: Multiple device types and platforms
2. **Feature Flags**: Maintain PC/SC fallback during transition
3. **Documentation**: Comprehensive migration and troubleshooting guides
4. **Community Feedback**: Early alpha releases for testing

## Success Criteria

### Technical Success
- [ ] All existing functionality works with USB CCID transport
- [ ] Static musl binaries compile successfully
- [ ] Performance matches or exceeds PC/SC implementation
- [ ] Cross-platform compatibility maintained

### User Experience Success
- [ ] Simplified deployment (single static binary)
- [ ] Reduced system dependencies
- [ ] Clear migration path and documentation
- [ ] Backward compatibility maintained

### Project Success
- [ ] Community adoption of new transport
- [ ] Successful static binary use cases
- [ ] Reduced support burden from dependency issues
- [ ] Foundation for future embedded implementations

## Timeline

**Total Duration**: 5 weeks

- **Week 1**: Foundation and setup
- **Week 2**: Core CCID implementation
- **Week 3**: Integration and feature completion
- **Week 4**: Testing and validation
- **Week 5**: Documentation and release preparation

## Resource Requirements

### Development
- 1 experienced Rust developer
- Access to tapsigner/satscard devices for testing
- Multiple platforms for cross-platform testing

### Hardware
- Various CCID-compatible smart card readers
- Test smart cards
- Development machines (Linux, macOS, Windows)

## Conclusion

This proposal provides a comprehensive plan for eliminating PC/SC dependencies while maintaining full API compatibility. The pure Rust implementation will enable true static binary compilation, simplify deployment, and provide a foundation for future embedded and cross-platform development.

The phased approach minimizes risk while ensuring thorough testing and validation. Feature flags provide a smooth migration path, allowing users to adopt the new implementation gradually while maintaining backward compatibility.

The investment in pure Rust implementation will pay dividends in deployment simplicity, maintenance burden reduction, and expanded platform support for the rust-cktap ecosystem.