use crate::usb_transport::{find_ccid_endpoints, UsbTransport};
use crate::{CkTapCard, CkTransport, Error};
use log::{debug, info};
use rusb::{Context, Device, DeviceDescriptor, DeviceHandle, UsbContext};

/// USB class code for Smart Card devices (CCID)
const USB_CLASS_SMART_CARD: u8 = 0x0B;

/// Known USB vendor/product IDs for Coinkite devices
const COINKITE_VENDOR_ID: u16 = 0xD13E;
const COINKITE_PRODUCTS: &[(u16, &str)] = &[
    (0xCC10, "TAPSIGNER"),
    (0x0100, "Mk1/Mk2"),
    // Add more Coinkite product IDs as needed
];

/// Information about a discovered CCID device
#[derive(Debug)]
pub struct CcidDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub is_coinkite: bool,
}

/// Find the first available CCID card reader and connect to it
pub async fn find_first() -> Result<CkTapCard<UsbTransport>, Error> {
    let context = Context::new().map_err(Error::Usb)?;

    info!("Searching for CCID devices...");

    for device in context.devices().map_err(Error::Usb)?.iter() {
        if let Ok(info) = get_device_info(&device) {
            if info.is_coinkite {
                info!("Found Coinkite device: {info:?}");

                if let Ok(transport) = open_ccid_device(&device) {
                    return transport.to_cktap().await;
                }
            }
        }
    }

    // If no Coinkite device found, try any CCID device
    // Prioritize certain readers that are known to work well
    let devices: Vec<_> = context.devices().map_err(Error::Usb)?.iter().collect();

    // First try OMNIKEY readers (known to work well)
    for device in &devices {
        if let Ok(info) = get_device_info(device) {
            if info.vendor_id == 0x076B {
                // OMNIKEY vendor ID
                info!("Trying OMNIKEY reader: {info:?}");

                match open_ccid_device(device) {
                    Ok(transport) => match transport.to_cktap().await {
                        Ok(card) => return Ok(card),
                        Err(e) => debug!("Failed to initialize card: {e}"),
                    },
                    Err(e) => debug!("Failed to open device: {e}"),
                }
            }
        }
    }

    // Then try other CCID devices
    for device in &devices {
        if is_ccid_device(device).unwrap_or(false) {
            if let Ok(info) = get_device_info(device) {
                // Skip YubiKey for now - it might not have a card inserted
                if info.vendor_id == 0x1050 {
                    debug!("Skipping YubiKey");
                    continue;
                }

                debug!("Trying generic CCID device: {info:?}");

                match open_ccid_device(device) {
                    Ok(transport) => match transport.to_cktap().await {
                        Ok(card) => return Ok(card),
                        Err(e) => debug!("Failed to initialize card: {e}"),
                    },
                    Err(e) => debug!("Failed to open device: {e}"),
                }
            }
        }
    }

    Err(Error::DeviceNotFound)
}

/// List all available CCID devices
pub fn list_devices() -> Result<Vec<CcidDeviceInfo>, Error> {
    let context = Context::new().map_err(Error::Usb)?;
    let mut devices = Vec::new();

    for device in context.devices().map_err(Error::Usb)?.iter() {
        if let Ok(info) = get_device_info(&device) {
            devices.push(info);
        }
    }

    Ok(devices)
}

/// Get information about a USB device
fn get_device_info(device: &Device<Context>) -> Result<CcidDeviceInfo, Error> {
    let desc = device.device_descriptor().map_err(Error::Usb)?;

    if !is_ccid_device_descriptor(&desc, device)? {
        return Err(Error::NotCcidDevice);
    }

    let handle = device.open().map_err(Error::Usb)?;

    let manufacturer = read_string_descriptor(&handle, &desc, desc.manufacturer_string_index());
    let product = read_string_descriptor(&handle, &desc, desc.product_string_index());
    let serial = read_string_descriptor(&handle, &desc, desc.serial_number_string_index());

    let is_coinkite = desc.vendor_id() == COINKITE_VENDOR_ID
        || COINKITE_PRODUCTS
            .iter()
            .any(|(pid, _)| desc.product_id() == *pid);

    Ok(CcidDeviceInfo {
        vendor_id: desc.vendor_id(),
        product_id: desc.product_id(),
        manufacturer,
        product,
        serial,
        is_coinkite,
    })
}

/// Check if a device is a CCID device
fn is_ccid_device(device: &Device<Context>) -> Result<bool, Error> {
    let desc = device.device_descriptor().map_err(Error::Usb)?;
    is_ccid_device_descriptor(&desc, device)
}

/// Check if a device descriptor indicates a CCID device
fn is_ccid_device_descriptor(
    desc: &DeviceDescriptor,
    device: &Device<Context>,
) -> Result<bool, Error> {
    // Check device class
    if desc.class_code() == USB_CLASS_SMART_CARD {
        return Ok(true);
    }

    // Check interface class
    let config = match device.active_config_descriptor() {
        Ok(config) => config,
        Err(_) => return Ok(false), // Can't read config, assume not CCID
    };

    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            if descriptor.class_code() == USB_CLASS_SMART_CARD {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Open a CCID device and create a transport
fn open_ccid_device(device: &Device<Context>) -> Result<UsbTransport, Error> {
    let handle = device.open().map_err(Error::Usb)?;

    // Find the CCID interface
    let config = device.active_config_descriptor().map_err(Error::Usb)?;

    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            if descriptor.class_code() == USB_CLASS_SMART_CARD {
                let interface_num = interface.number();

                // Detach kernel driver if needed (Linux)
                #[cfg(target_os = "linux")]
                {
                    if handle.kernel_driver_active(interface_num).unwrap_or(false) {
                        handle.detach_kernel_driver(interface_num).ok();
                    }
                }

                // Claim the interface
                handle.claim_interface(interface_num).map_err(Error::Usb)?;

                // Find endpoints
                let (endpoint_out, endpoint_in) = find_ccid_endpoints(&handle, interface_num)?;

                info!(
                    "Opened CCID device on interface {interface_num} (endpoints: out={endpoint_out:#x}, in={endpoint_in:#x})"
                );

                return Ok(UsbTransport::new(
                    handle,
                    interface_num,
                    endpoint_out,
                    endpoint_in,
                ));
            }
        }
    }

    Err(Error::Ccid("No CCID interface found".to_string()))
}

/// Read a string descriptor from a device
fn read_string_descriptor(
    handle: &DeviceHandle<Context>,
    _desc: &DeviceDescriptor,
    index: Option<u8>,
) -> Option<String> {
    match index {
        Some(idx) if idx > 0 => handle.read_string_descriptor_ascii(idx).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coinkite_detection() {
        // Test known Coinkite vendor ID
        assert_eq!(COINKITE_VENDOR_ID, 0xD13E);

        // Test product lookup
        assert!(COINKITE_PRODUCTS
            .iter()
            .any(|(pid, name)| { *pid == 0xCC10 && *name == "TAPSIGNER" }));
    }
}
