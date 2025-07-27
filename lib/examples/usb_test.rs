use rust_cktap::discovery;
use rust_cktap::CkTapCard;

#[tokio::main]
async fn main() -> Result<(), rust_cktap::Error> {
    env_logger::init();

    println!("Searching for CCID devices...");

    // List all devices
    match discovery::list_devices() {
        Ok(devices) => {
            println!("Found {} CCID device(s):", devices.len());
            for device in devices {
                println!(
                    "  - Vendor: {:#06x}, Product: {:#06x}",
                    device.vendor_id, device.product_id
                );
                if let Some(manufacturer) = device.manufacturer {
                    println!("    Manufacturer: {}", manufacturer);
                }
                if let Some(product) = device.product {
                    println!("    Product: {}", product);
                }
                if let Some(serial) = device.serial {
                    println!("    Serial: {}", serial);
                }
                println!("    Coinkite device: {}", device.is_coinkite);
            }
        }
        Err(e) => {
            eprintln!("Error listing devices: {}", e);
        }
    }

    println!("\nConnecting to first available card...");

    // Connect to first card
    let card = match discovery::find_first().await {
        Ok(card) => card,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nMake sure:");
            eprintln!("- Your card reader is connected");
            eprintln!("- You have permissions to access USB devices");
            eprintln!("- On Linux, you may need to add udev rules or run as root");
            return Err(e);
        }
    };

    println!("Successfully connected!");
    println!("Card type: {:?}", card);

    // Try to get status
    match card {
        CkTapCard::TapSigner(mut ts) => {
            println!("\nTapSigner detected!");
            match ts.status().await {
                Ok(status) => {
                    println!("Status:");
                    println!("  Protocol: {}", status.proto);
                    println!("  Version: {}", status.ver);
                    println!("  Birth: {}", status.birth);
                    if let Some(path) = status.path {
                        println!("  Path: {:?}", path);
                    }
                    println!("  Card nonce: {:02x?}", status.card_nonce);
                }
                Err(e) => {
                    eprintln!("Error getting status: {}", e);
                }
            }
        }
        CkTapCard::SatsCard(mut sc) => {
            println!("\nSatsCard detected!");
            println!("Card details:");
            println!("  Protocol: {}", sc.proto);
            println!("  Version: {}", sc.ver);
            println!("  Birth: {}", sc.birth);
            println!("  Slots: {:?}", sc.slots);
            if let Some(addr) = &sc.addr {
                println!("  Address: {}", addr);
            }
        }
        CkTapCard::SatsChip(mut ts) => {
            println!("\nSatsChip detected!");
            match ts.status().await {
                Ok(status) => {
                    println!("Status:");
                    println!("  Protocol: {}", status.proto);
                    println!("  Version: {}", status.ver);
                    println!("  Birth: {}", status.birth);
                }
                Err(e) => {
                    eprintln!("Error getting status: {}", e);
                }
            }
        }
    }

    Ok(())
}
