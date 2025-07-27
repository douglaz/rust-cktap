use cktap_direct::CkTapCard;
use cktap_direct::discovery;

#[tokio::main]
async fn main() -> Result<(), cktap_direct::Error> {
    env_logger::init();

    println!("Searching for CCID devices...");

    // List all devices
    match discovery::list_devices() {
        Ok(devices) => {
            println!("Found {count} CCID device(s):", count = devices.len());
            for device in devices {
                println!(
                    "  - Vendor: {vendor:#06x}, Product: {product:#06x}",
                    vendor = device.vendor_id,
                    product = device.product_id
                );
                if let Some(manufacturer) = device.manufacturer {
                    println!("    Manufacturer: {manufacturer}");
                }
                if let Some(product) = device.product {
                    println!("    Product: {product}");
                }
                if let Some(serial) = device.serial {
                    println!("    Serial: {serial}");
                }
                println!(
                    "    Coinkite device: {is_coinkite}",
                    is_coinkite = device.is_coinkite
                );
            }
        }
        Err(e) => {
            eprintln!("Error listing devices: {e}");
        }
    }

    println!("\nConnecting to first available card...");

    // Connect to first card
    let card = match discovery::find_first().await {
        Ok(card) => card,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("\nMake sure:");
            eprintln!("- Your card reader is connected");
            eprintln!("- You have permissions to access USB devices");
            eprintln!("- On Linux, you may need to add udev rules or run as root");
            return Err(e);
        }
    };

    println!("Successfully connected!");
    println!("Card type: {card:?}");

    // Try to get status
    match card {
        CkTapCard::TapSigner(mut ts) => {
            println!("\nTapSigner detected!");
            match ts.status().await {
                Ok(status) => {
                    println!("Status:");
                    println!("  Protocol: {proto}", proto = status.proto);
                    println!("  Version: {ver}", ver = status.ver);
                    println!("  Birth: {birth}", birth = status.birth);
                    if let Some(path) = status.path {
                        println!("  Path: {path:?}");
                    }
                    println!("  Card nonce: {nonce:02x?}", nonce = status.card_nonce);
                }
                Err(e) => {
                    eprintln!("Error getting status: {e}");
                }
            }
        }
        CkTapCard::SatsCard(sc) => {
            println!("\nSatsCard detected!");
            println!("Card details:");
            println!("  Protocol: {proto}", proto = sc.proto);
            println!("  Version: {ver}", ver = sc.ver);
            println!("  Birth: {birth}", birth = sc.birth);
            println!("  Slots: {slots:?}", slots = sc.slots);
            if let Some(addr) = &sc.addr {
                println!("  Address: {addr}");
            }
        }
        CkTapCard::SatsChip(mut ts) => {
            println!("\nSatsChip detected!");
            match ts.status().await {
                Ok(status) => {
                    println!("Status:");
                    println!("  Protocol: {proto}", proto = status.proto);
                    println!("  Version: {ver}", ver = status.ver);
                    println!("  Birth: {birth}", birth = status.birth);
                }
                Err(e) => {
                    eprintln!("Error getting status: {e}");
                }
            }
        }
    }

    Ok(())
}
