use cktap_direct::commands::{CkTransport, Read};
#[cfg(not(feature = "emulator"))]
use cktap_direct::discovery;
#[cfg(feature = "emulator")]
use cktap_direct::emulator;
use cktap_direct::secp256k1::hashes::{hex::DisplayHex, Hash as _};
use cktap_direct::secp256k1::rand;
use cktap_direct::{
    apdu::{CkTapError, Error},
    commands::Certificate,
    rand_chaincode, CkTapCard,
};
/// CLI for cktap-direct
use clap::{Parser, Subcommand};
use rpassword::read_password;
use std::io;
use std::io::Write;

/// SatsCard CLI
#[derive(Parser)]
#[command(author, version = option_env ! ("CARGO_PKG_VERSION").unwrap_or("unknown"), about,
long_about = None, propagate_version = true)]
struct SatsCardCli {
    #[command(subcommand)]
    command: SatsCardCommand,
}

/// Commands supported by SatsCard cards
#[derive(Subcommand)]
enum SatsCardCommand {
    /// Show the card status
    Debug,
    /// Show current deposit address
    Address,
    /// Check this card was made by Coinkite: Verifies a certificate chain up to root factory key.
    Certs,
    /// Read the pubkey
    Read,
    /// Pick a new private key and start a fresh slot. Current slot must be unsealed.
    New,
    /// Unseal the current slot.
    Unseal,
    /// Get the payment address and verify it follows from the chain code and master public key
    Derive,
}

/// TapSigner CLI
#[derive(Parser)]
#[command(author, version = option_env ! ("CARGO_PKG_VERSION").unwrap_or("unknown"), about,
long_about = None, propagate_version = true)]
struct TapSignerCli {
    #[command(subcommand)]
    command: TapSignerCommand,
}

/// Commands supported by TapSigner cards
#[derive(Subcommand)]
enum TapSignerCommand {
    /// Show the card status
    Debug,
    /// Check this card was made by Coinkite: Verifies a certificate chain up to root factory key.
    Certs,
    /// Read the pubkey (requires CVC)
    Read,
    /// This command is used once to initialize a new card.
    Init,
    /// Derive a public key at the given hardened path
    Derive {
        /// path, eg. for 84'/0'/0'/* use 84,0,0
        #[clap(short, long, value_delimiter = ',', num_args = 1..)]
        path: Vec<u32>,
    },
    /// Get an encrypted backup of the card's private key
    Backup,
    /// Change the PIN (CVC) used for card authentication to a new user provided one
    Change { new_cvc: String },
    /// Sign a digest
    Sign { to_sign: String },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Parse CLI args first to see what command we're running
    let args: Vec<String> = std::env::args().collect();
    let needs_cvc = args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "read" | "init" | "derive" | "backup" | "change" | "sign" | "new" | "unseal"
        )
    });

    // Get CVC before connecting to device if needed
    let cvc_value = if needs_cvc {
        Some(get_cvc_from_env_or_prompt())
    } else {
        None
    };

    // figure out what type of card we have before parsing cli args
    #[cfg(not(feature = "emulator"))]
    let mut card = discovery::find_first().await?;

    // if emulator feature enabled override pcsc card
    #[cfg(feature = "emulator")]
    let mut card = emulator::find_emulator().await?;

    let rng = &mut rand::thread_rng();

    match &mut card {
        CkTapCard::SatsCard(sc) => {
            let cli = SatsCardCli::parse();
            match cli.command {
                SatsCardCommand::Debug => {
                    dbg!(&sc);
                }
                SatsCardCommand::Address => {
                    let address = sc.address().await?;
                    println!("Address: {address}");
                }
                SatsCardCommand::Certs => check_cert(sc).await,
                SatsCardCommand::Read => read(sc, None).await,
                SatsCardCommand::New => {
                    let slot = sc
                        .slot()
                        .ok_or_else(|| Error::UnknownCardType("No available slot".to_string()))?;
                    let chain_code = Some(rand_chaincode(rng));
                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    let response = sc.new_slot(slot, chain_code, cvc).await?;
                    println!("{response}")
                }
                SatsCardCommand::Unseal => {
                    let slot = sc
                        .slot()
                        .ok_or_else(|| Error::UnknownCardType("No available slot".to_string()))?;
                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    let response = sc.unseal(slot, cvc).await?;
                    println!("{response}")
                }
                SatsCardCommand::Derive => {
                    dbg!(&sc.derive().await);
                }
            }
        }
        CkTapCard::TapSigner(ts) | CkTapCard::SatsChip(ts) => {
            let cli = TapSignerCli::parse();
            match cli.command {
                TapSignerCommand::Debug => {
                    dbg!(&ts);
                }
                TapSignerCommand::Certs => check_cert(ts).await,
                TapSignerCommand::Read => read(ts, cvc_value.clone()).await,
                TapSignerCommand::Init => {
                    let chain_code = rand_chaincode(rng);
                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    let response = ts.init(chain_code, cvc).await;
                    dbg!(&response);
                }
                TapSignerCommand::Derive { path } => {
                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    match &ts.derive(&path, cvc).await {
                        Ok(response) => {
                            println!(
                                "Derived public key at path m/{}:",
                                path.iter()
                                    .map(|&p| format!("{p}'"))
                                    .collect::<Vec<_>>()
                                    .join("/")
                            );

                            let pubkey_hex =
                                response.pubkey.as_ref().unwrap_or(&response.master_pubkey);
                            let pubkey_hex_str = pubkey_hex.as_hex();
                            println!("Public key: {pubkey_hex_str}");

                            // Convert to Bitcoin address (assuming native segwit for BIP84)
                            if !path.is_empty() && path[0] == 84 {
                                if let Ok(pubkey) = bitcoin::PublicKey::from_slice(pubkey_hex) {
                                    if let Ok(compressed) =
                                        bitcoin::CompressedPublicKey::try_from(pubkey)
                                    {
                                        let address = bitcoin::Address::p2wpkh(
                                            &compressed,
                                            bitcoin::Network::Bitcoin,
                                        );
                                        println!("Bitcoin address (mainnet): {address}");

                                        let testnet_address = bitcoin::Address::p2wpkh(
                                            &compressed,
                                            bitcoin::Network::Testnet,
                                        );
                                        println!("Bitcoin address (testnet): {testnet_address}");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error deriving key: {e:?}");
                        }
                    }
                }

                TapSignerCommand::Backup => {
                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    let response = ts.backup(cvc).await;
                    println!("{response:?}");
                }

                TapSignerCommand::Change { new_cvc } => {
                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    let response = ts.change(&new_cvc, cvc).await;
                    println!("{response:?}");
                }
                TapSignerCommand::Sign { to_sign } => {
                    let digest: [u8; 32] =
                        cktap_direct::secp256k1::hashes::sha256::Hash::hash(to_sign.as_bytes())
                            .to_byte_array();

                    let cvc = cvc_value
                        .as_ref()
                        .ok_or(Error::CkTap(CkTapError::NeedsAuth))?;
                    let response = ts.sign(digest, vec![], cvc).await;
                    println!("{response:?}");
                }
            }
        }
    }

    Ok(())
}

// handler functions for each command

async fn check_cert<C, T: CkTransport>(card: &mut C)
where
    C: Certificate<T>,
{
    if let Ok(k) = card.check_certificate().await {
        println!(
            "Genuine card from Coinkite.\nHas cert signed by: {}",
            k.name()
        )
    } else {
        println!("Card failed to verify. Not a genuine card")
    }
}

async fn read<C, T: CkTransport>(card: &mut C, cvc: Option<String>)
where
    C: Read<T>,
{
    match card.read(cvc).await {
        Ok(resp) => println!("{resp}"),
        Err(e) => {
            dbg!(&e);
            println!("Failed to read with error: ")
        }
    }
}

fn cvc() -> Result<String, std::io::Error> {
    print!("Enter cvc: ");
    io::stdout().flush()?;
    let cvc = read_password()?;
    Ok(cvc.trim().to_string())
}

fn get_cvc_from_env_or_prompt() -> String {
    match std::env::var("CKTAP_CVC") {
        Ok(cvc) => cvc,
        Err(_) => cvc().unwrap_or_else(|_| {
            eprintln!("Failed to read CVC from terminal");
            std::process::exit(1);
        }),
    }
}
