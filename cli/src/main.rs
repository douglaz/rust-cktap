mod output;

use anyhow::{Context, Result};
use cktap_direct::commands::{CkTransport, Read};
#[cfg(not(feature = "emulator"))]
use cktap_direct::discovery;
#[cfg(feature = "emulator")]
use cktap_direct::emulator;
use cktap_direct::secp256k1::hashes::{hex::DisplayHex, Hash as _};
use cktap_direct::secp256k1::rand;
use cktap_direct::{commands::Certificate, rand_chaincode, CkTapCard};
use clap::{Parser, Subcommand};
use output::*;
use rpassword::read_password;
use std::io;
use std::io::Write;

/// CLI for cktap-direct - interact with Coinkite TapSigner and SatsCard devices
#[derive(Parser)]
#[command(author, version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown"), about, long_about = None, propagate_version = true)]
struct Cli {
    /// Output format
    #[arg(long, value_parser = clap::value_parser!(OutputFormat), default_value = "json", global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// SatsCard-specific commands
    #[command(subcommand)]
    Satscard(SatsCardCommand),

    /// TapSigner-specific commands
    #[command(subcommand)]
    Tapsigner(TapSignerCommand),

    /// Auto-detect card type and run command
    #[command(subcommand)]
    Auto(AutoCommand),
}

/// Commands that work with any card type
#[derive(Subcommand)]
enum AutoCommand {
    /// Show the card status
    Status,
    /// Check this card was made by Coinkite
    Certs,
}

/// Commands supported by SatsCard cards
#[derive(Subcommand)]
enum SatsCardCommand {
    /// Show the card status
    Status,
    /// Show current deposit address
    Address,
    /// Check this card was made by Coinkite
    Certs,
    /// Read the pubkey
    Read,
    /// Pick a new private key and start a fresh slot
    New,
    /// Unseal the current slot
    Unseal,
    /// Get the payment address and verify it
    Derive,
}

/// Commands supported by TapSigner cards
#[derive(Subcommand)]
enum TapSignerCommand {
    /// Show the card status
    Status,
    /// Check this card was made by Coinkite
    Certs,
    /// Read the pubkey (requires CVC)
    Read,
    /// Initialize a new card
    Init,
    /// Derive a public key at the given hardened path
    Derive {
        /// Derivation path components (e.g., 84,0,0 for m/84'/0'/0')
        #[clap(short, long, value_delimiter = ',', num_args = 1..)]
        path: Vec<u32>,
    },
    /// Get an encrypted backup of the card's private key
    Backup,
    /// Change the PIN (CVC) used for card authentication
    Change {
        /// New CVC/PIN to set
        new_cvc: String,
    },
    /// Sign a digest
    Sign {
        /// Data to sign (will be hashed with SHA256)
        to_sign: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    // Connect to card
    #[cfg(not(feature = "emulator"))]
    let card = discovery::find_first()
        .await
        .context("Failed to find card")?;

    #[cfg(feature = "emulator")]
    let card = emulator::find_emulator()
        .await
        .context("Failed to connect to emulator")?;

    match cli.command {
        Commands::Auto(cmd) => handle_auto_command(card, cmd, cli.format).await,
        Commands::Satscard(cmd) => handle_satscard_command(card, cmd, cli.format).await,
        Commands::Tapsigner(cmd) => handle_tapsigner_command(card, cmd, cli.format).await,
    }
}

async fn handle_auto_command<T: CkTransport>(
    mut card: CkTapCard<T>,
    command: AutoCommand,
    format: OutputFormat,
) -> Result<()> {
    match command {
        AutoCommand::Status => {
            let response = match &card {
                CkTapCard::SatsCard(sc) => {
                    let slots = SlotInfo {
                        current: sc.slots.0,
                        total: sc.slots.1,
                    };
                    DebugResponse {
                        card_type: "satscard".to_string(),
                        card_ident: format!(
                            "CARD-{:X}",
                            sc.pubkey.serialize()[0..4]
                                .iter()
                                .fold(0u32, |acc, &b| (acc << 8) | b as u32)
                        ),
                        birth_height: Some(sc.birth as u32),
                        slots: Some(slots),
                        path: None,
                        applet_version: sc.ver.clone(),
                        is_testnet: false,
                    }
                }
                CkTapCard::TapSigner(ts) | CkTapCard::SatsChip(ts) => DebugResponse {
                    card_type: if matches!(card, CkTapCard::SatsChip(_)) {
                        "satschip".to_string()
                    } else {
                        "tapsigner".to_string()
                    },
                    card_ident: format!(
                        "CARD-{:X}",
                        ts.pubkey.serialize()[0..4]
                            .iter()
                            .fold(0u32, |acc, &b| (acc << 8) | b as u32)
                    ),
                    birth_height: Some(ts.birth as u32),
                    slots: None,
                    path: ts
                        .path
                        .as_ref()
                        .map(|p| p.iter().map(|&v| v as u32).collect()),
                    applet_version: ts.ver.clone(),
                    is_testnet: false,
                },
            };
            output_response(success_response(response), format)?;
        }
        AutoCommand::Certs => {
            let result = match &mut card {
                CkTapCard::SatsCard(sc) => check_cert(sc).await,
                CkTapCard::TapSigner(ts) | CkTapCard::SatsChip(ts) => check_cert(ts).await,
            };
            output_response(result, format)?;
        }
    }
    Ok(())
}

async fn handle_satscard_command<T: CkTransport>(
    card: CkTapCard<T>,
    command: SatsCardCommand,
    format: OutputFormat,
) -> Result<()> {
    let mut sc = match card {
        CkTapCard::SatsCard(sc) => sc,
        _ => anyhow::bail!("Connected card is not a SatsCard"),
    };

    let rng = &mut rand::thread_rng();

    match command {
        SatsCardCommand::Status => {
            let slots = SlotInfo {
                current: sc.slots.0,
                total: sc.slots.1,
            };
            let response = DebugResponse {
                card_type: "satscard".to_string(),
                card_ident: format!(
                    "CARD-{:X}",
                    sc.pubkey.serialize()[0..4]
                        .iter()
                        .fold(0u32, |acc, &b| (acc << 8) | b as u32)
                ),
                birth_height: Some(sc.birth as u32),
                slots: Some(slots),
                path: None,
                applet_version: sc.ver.clone(),
                is_testnet: false, // TODO: check if card is testnet
            };
            output_response(success_response(response), format)?;
        }
        SatsCardCommand::Address => {
            let address = sc.address().await.context("Failed to get address")?;
            let response = AddressResponse { address };
            output_response(success_response(response), format)?;
        }
        SatsCardCommand::Certs => {
            let result = check_cert(&mut sc).await;
            output_response(result, format)?;
        }
        SatsCardCommand::Read => {
            let result = read_card(&mut sc, None).await;
            output_response(result, format)?;
        }
        SatsCardCommand::New => {
            let slot = sc.slot().context("No available slot")?;
            let chain_code = Some(rand_chaincode(rng));
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;

            let response = sc
                .new_slot(slot, chain_code, &cvc)
                .await
                .context("Failed to create new slot")?;

            let result = NewSlotResponse {
                slot: response.slot,
            };
            output_response(success_response(result), format)?;
        }
        SatsCardCommand::Unseal => {
            let slot = sc.slot().context("No available slot")?;
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;

            let response = sc
                .unseal(slot, &cvc)
                .await
                .context("Failed to unseal slot")?;

            let result = UnsealResponse {
                slot: response.slot,
                master_pk: response.master_pk.as_hex().to_string(),
                pubkey: response.pubkey.as_hex().to_string(),
                privkey: response.privkey.as_hex().to_string(),
                chain_code: if response.chain_code.is_empty() {
                    None
                } else {
                    Some(response.chain_code.as_hex().to_string())
                },
            };
            output_response(success_response(result), format)?;
        }
        SatsCardCommand::Derive => {
            let response = sc.derive().await.context("Failed to derive")?;

            // For SatsCard, derive returns verification that the payment address
            // follows from the chain code and master public key
            let result = DeriveResponse {
                path: "m".to_string(), // SatsCard uses master key
                pubkey: response
                    .pubkey
                    .map(|pk| pk.as_hex().to_string())
                    .unwrap_or_else(|| response.master_pubkey.as_hex().to_string()),
                master_pubkey: Some(response.master_pubkey.as_hex().to_string()),
                chain_code: Some(response.chain_code.as_hex().to_string()),
                addresses: None, // SatsCard derive doesn't compute addresses
            };
            output_response(success_response(result), format)?;
        }
    }
    Ok(())
}

async fn handle_tapsigner_command<T: CkTransport>(
    card: CkTapCard<T>,
    command: TapSignerCommand,
    format: OutputFormat,
) -> Result<()> {
    let mut ts = match card {
        CkTapCard::TapSigner(ts) | CkTapCard::SatsChip(ts) => ts,
        _ => anyhow::bail!("Connected card is not a TapSigner"),
    };

    let rng = &mut rand::thread_rng();

    match command {
        TapSignerCommand::Status => {
            let response = DebugResponse {
                card_type: "tapsigner".to_string(),
                card_ident: format!(
                    "CARD-{:X}",
                    ts.pubkey.serialize()[0..4]
                        .iter()
                        .fold(0u32, |acc, &b| (acc << 8) | b as u32)
                ),
                birth_height: Some(ts.birth as u32),
                slots: None,
                path: ts
                    .path
                    .as_ref()
                    .map(|p| p.iter().map(|&v| v as u32).collect()),
                applet_version: ts.ver.clone(),
                is_testnet: false, // TODO: check if card is testnet
            };
            output_response(success_response(response), format)?;
        }
        TapSignerCommand::Certs => {
            let result = check_cert(&mut ts).await;
            output_response(result, format)?;
        }
        TapSignerCommand::Read => {
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;
            let result = read_card(&mut ts, Some(cvc)).await;
            output_response(result, format)?;
        }
        TapSignerCommand::Init => {
            let chain_code = rand_chaincode(rng);
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;

            let _response = ts
                .init(chain_code, &cvc)
                .await
                .context("Failed to initialize card")?;

            let result = InitResponse {
                card_ident: format!(
                    "CARD-{:X}",
                    ts.pubkey.serialize()[0..4]
                        .iter()
                        .fold(0u32, |acc, &b| (acc << 8) | b as u32)
                ),
                success: true,
            };
            output_response(success_response(result), format)?;
        }
        TapSignerCommand::Derive { path } => {
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;

            let response = ts
                .derive(&path, &cvc)
                .await
                .context("Failed to derive key")?;

            let pubkey_hex = response.pubkey.as_ref().unwrap_or(&response.master_pubkey);

            let mut addresses = std::collections::HashMap::new();

            // Convert to Bitcoin address if BIP84 path
            if !path.is_empty() && path[0] == 84 {
                if let Ok(pubkey) = bitcoin::PublicKey::from_slice(pubkey_hex) {
                    if let Ok(compressed) = bitcoin::CompressedPublicKey::try_from(pubkey) {
                        let mainnet_addr =
                            bitcoin::Address::p2wpkh(&compressed, bitcoin::Network::Bitcoin);
                        let testnet_addr =
                            bitcoin::Address::p2wpkh(&compressed, bitcoin::Network::Testnet);
                        addresses.insert("mainnet".to_string(), mainnet_addr.to_string());
                        addresses.insert("testnet".to_string(), testnet_addr.to_string());
                    }
                }
            }

            let path_str = path
                .iter()
                .map(|&p| format!("{p}'"))
                .collect::<Vec<_>>()
                .join("/");

            let result = DeriveResponse {
                path: format!("m/{path_str}"),
                pubkey: pubkey_hex.as_hex().to_string(),
                master_pubkey: Some(response.master_pubkey.as_hex().to_string()),
                chain_code: Some(response.chain_code.as_hex().to_string()),
                addresses: if addresses.is_empty() {
                    None
                } else {
                    Some(addresses)
                },
            };
            output_response(success_response(result), format)?;
        }
        TapSignerCommand::Backup => {
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;

            let response = ts.backup(&cvc).await.context("Failed to create backup")?;

            let result = BackupResponse {
                data: response.data.as_hex().to_string(),
                written: response.data.len() as u8,
            };
            output_response(success_response(result), format)?;
        }
        TapSignerCommand::Change { new_cvc } => {
            let cvc = get_cvc_from_env_or_prompt().context("Failed to get current CVC")?;

            let response = ts
                .change(&new_cvc, &cvc)
                .await
                .context("Failed to change CVC")?;

            let result = ChangeResponse {
                success: response.success,
                delay_seconds: None,
            };
            output_response(success_response(result), format)?;
        }
        TapSignerCommand::Sign { to_sign } => {
            let digest: [u8; 32] =
                cktap_direct::secp256k1::hashes::sha256::Hash::hash(to_sign.as_bytes())
                    .to_byte_array();

            let cvc = get_cvc_from_env_or_prompt().context("Failed to get CVC")?;

            let response = ts
                .sign(digest, vec![], &cvc)
                .await
                .context("Failed to sign")?;

            let result = SignResponse {
                signature: response.sig.as_hex().to_string(),
                pubkey: response.pubkey.as_hex().to_string(),
            };
            output_response(success_response(result), format)?;
        }
    }
    Ok(())
}

async fn check_cert<C, T>(card: &mut C) -> CommandResponse<CertsResponse>
where
    C: Certificate<T>,
    T: CkTransport,
{
    match card.check_certificate().await {
        Ok(key) => {
            let response = CertsResponse {
                genuine: true,
                signed_by: Some(key.name().to_string()),
                message: Some("Genuine card from Coinkite".to_string()),
            };
            success_response(response)
        }
        Err(e) => {
            let response = CertsResponse {
                genuine: false,
                signed_by: None,
                message: Some("Card failed to verify. Not a genuine card".to_string()),
            };
            CommandResponse {
                success: false,
                error: Some(e.to_string()),
                data: Some(response),
            }
        }
    }
}

async fn read_card<C, T>(card: &mut C, cvc: Option<String>) -> CommandResponse<ReadResponse>
where
    C: Read<T>,
    T: CkTransport,
{
    match card.read(cvc).await {
        Ok(resp) => {
            let response = ReadResponse {
                pubkey: resp.pubkey.as_hex().to_string(),
                card_nonce: Some(resp.card_nonce.as_hex().to_string()),
                signature: Some(resp.sig.as_hex().to_string()),
            };
            success_response(response)
        }
        Err(e) => error_response(e.to_string()),
    }
}

fn cvc() -> Result<String> {
    eprint!("Enter CVC: ");
    io::stderr().flush()?;
    let cvc = read_password()?;
    Ok(cvc.trim().to_string())
}

fn get_cvc_from_env_or_prompt() -> Result<String> {
    match std::env::var("CKTAP_CVC") {
        Ok(cvc) => Ok(cvc),
        Err(_) => cvc(),
    }
}
