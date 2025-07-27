use crate::Error;
use crate::ccid::{CcidCommand, CcidResponse, SlotError, SlotStatus, VoltageSelection};
use crate::commands::CkTransport;
use rusb::{Context, DeviceHandle};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

/// USB CCID transport implementation
pub struct UsbTransport {
    device: DeviceHandle<Context>,
    interface: u8,
    endpoint_out: u8,
    endpoint_in: u8,
    sequence: AtomicU8,
    timeout: Duration,
}

impl UsbTransport {
    /// Create a new USB transport from a device handle
    pub fn new(
        device: DeviceHandle<Context>,
        interface: u8,
        endpoint_out: u8,
        endpoint_in: u8,
    ) -> Self {
        Self {
            device,
            interface,
            endpoint_out,
            endpoint_in,
            sequence: AtomicU8::new(0),
            timeout: Duration::from_secs(5),
        }
    }

    /// Power on the card and get ATR
    pub async fn power_on(&self) -> Result<Vec<u8>, Error> {
        let sequence = self.next_sequence();
        let cmd = CcidCommand::icc_power_on(0, sequence, VoltageSelection::Automatic);

        self.send_command(cmd).await?;
        let response = self.read_response().await?;

        self.check_response_status(&response)?;

        // Response data contains the ATR
        Ok(response.data)
    }

    /// Send a CCID command
    async fn send_command(&self, cmd: CcidCommand) -> Result<(), Error> {
        let bytes = cmd.to_bytes();

        log::debug!(
            "Sending CCID command: type={:#x}, len={}, seq={}",
            cmd.header.message_type,
            bytes.len(),
            cmd.header.sequence
        );
        log::trace!("Command bytes: {bytes:02x?}");

        self.device
            .write_bulk(self.endpoint_out, &bytes, self.timeout)
            .map_err(Error::Usb)?;

        Ok(())
    }

    /// Read a CCID response
    async fn read_response(&self) -> Result<CcidResponse, Error> {
        let mut buffer = vec![0u8; 1024];

        let len = self
            .device
            .read_bulk(self.endpoint_in, &mut buffer, self.timeout)
            .map_err(Error::Usb)?;

        log::debug!("Received {len} bytes");
        log::trace!("Response bytes: {:02x?}", &buffer[..len.min(64)]);

        if len < 10 {
            return Err(Error::Ccid("Response too short".to_string()));
        }

        buffer.truncate(len);
        let response = CcidResponse::from_bytes(&buffer).map_err(|e| Error::Ccid(e.to_string()))?;

        log::debug!(
            "CCID response: type={:#x}, status={:?}, error={:?}",
            response.header.message_type,
            response.slot_status,
            response.slot_error
        );

        Ok(response)
    }

    /// Check response status and convert to error if needed
    fn check_response_status(&self, response: &CcidResponse) -> Result<(), Error> {
        match response.slot_error {
            SlotError::NoError => Ok(()),
            SlotError::CommandError => {
                // For DataBlock responses, error code is in the first byte after the 10-byte header
                // But for other responses, there might be no data
                log::debug!(
                    "CCID command error, slot status: {:?}, data len: {}",
                    response.slot_status,
                    response.data.len()
                );

                if response.slot_status == SlotStatus::NoICCPresent {
                    Err(Error::Ccid("No card present".to_string()))
                } else if response.data.is_empty() {
                    // Some errors don't have additional data
                    Err(Error::Ccid("Command error".to_string()))
                } else {
                    match response.data[0] {
                        0xFF => Err(Error::Ccid("Command aborted".to_string())),
                        0xFE => Err(Error::Ccid("ICC mute (no card?)".to_string())),
                        0xFD => Err(Error::Ccid("XFR parity error".to_string())),
                        0xFC => Err(Error::Ccid("XFR overrun".to_string())),
                        code => Err(Error::Ccid(format!("Command error: {code:#x}"))),
                    }
                }
            }
            SlotError::MoreTime => {
                log::debug!("Time extension requested");
                Err(Error::Ccid("Time extension requested".to_string()))
            }
            SlotError::HardwareError => Err(Error::Ccid("Hardware error".to_string())),
        }
    }

    /// Get the next sequence number
    fn next_sequence(&self) -> u8 {
        self.sequence.fetch_add(1, Ordering::Relaxed)
    }
}

impl CkTransport for UsbTransport {
    async fn transmit_apdu(&self, apdu: Vec<u8>) -> Result<Vec<u8>, Error> {
        // Always try to power on first - this is safer than checking status
        // If already powered on, this is typically a no-op
        match self.power_on().await {
            Ok(_) => {
                // Card powered on successfully
            }
            Err(e) => {
                // Log but don't fail - card might already be powered on
                log::debug!("Power on returned: {e}");
            }
        }

        // Send APDU via XfrBlock command
        let sequence = self.next_sequence();
        let cmd = CcidCommand::xfr_block(0, sequence, apdu);

        self.send_command(cmd).await?;
        let response = self.read_response().await?;

        self.check_response_status(&response)?;

        // Response data contains the R-APDU
        Ok(response.data)
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        // Release the interface when dropping
        let _ = self.device.release_interface(self.interface);
    }
}

/// Find CCID endpoints in a device interface
pub fn find_ccid_endpoints(
    device: &DeviceHandle<Context>,
    interface: u8,
) -> Result<(u8, u8), Error> {
    let config = device
        .device()
        .active_config_descriptor()
        .map_err(Error::Usb)?;

    let interface_desc = config
        .interfaces()
        .nth(interface as usize)
        .ok_or_else(|| Error::Ccid("Interface not found".to_string()))?
        .descriptors()
        .next()
        .ok_or_else(|| Error::Ccid("No interface descriptor".to_string()))?;

    let mut endpoint_in = None;
    let mut endpoint_out = None;

    for endpoint in interface_desc.endpoint_descriptors() {
        if endpoint.transfer_type() == rusb::TransferType::Bulk {
            match endpoint.direction() {
                rusb::Direction::In => endpoint_in = Some(endpoint.address()),
                rusb::Direction::Out => endpoint_out = Some(endpoint.address()),
            }
        }
    }

    match (endpoint_in, endpoint_out) {
        (Some(ep_in), Some(ep_out)) => Ok((ep_out, ep_in)),
        _ => Err(Error::Ccid("CCID bulk endpoints not found".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusb::UsbContext;

    #[test]
    fn test_sequence_counter() {
        let ctx = Context::new().expect("Failed to create USB context");
        let _devices = ctx.devices().expect("Failed to enumerate USB devices");

        // This test would need a mock device handle
        // For now, just test the sequence counter logic
        let sequence = AtomicU8::new(0);

        assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 0);
        assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 1);
        assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 2);

        // Test wraparound
        sequence.store(255, Ordering::Relaxed);
        assert_eq!(sequence.fetch_add(1, Ordering::Relaxed), 255);
        assert_eq!(sequence.load(Ordering::Relaxed), 0);
    }
}
