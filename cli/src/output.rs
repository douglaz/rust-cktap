use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Plain,
}

/// Generic command response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

/// Address command response
#[derive(Debug, Serialize, Deserialize)]
pub struct AddressResponse {
    pub address: String,
}

/// Certificate verification response
#[derive(Debug, Serialize, Deserialize)]
pub struct CertsResponse {
    pub genuine: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Read command response
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadResponse {
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// New slot response
#[derive(Debug, Serialize, Deserialize)]
pub struct NewSlotResponse {
    pub slot: u8,
}

/// Unseal response
#[derive(Debug, Serialize, Deserialize)]
pub struct UnsealResponse {
    pub slot: u8,
    pub master_pk: String,
    pub pubkey: String,
    pub privkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_code: Option<String>,
}

/// Derive response
#[derive(Debug, Serialize, Deserialize)]
pub struct DeriveResponse {
    pub path: String,
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<HashMap<String, String>>,
}

/// Init response
#[derive(Debug, Serialize, Deserialize)]
pub struct InitResponse {
    pub card_ident: String,
    pub success: bool,
}

/// Backup response
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupResponse {
    pub data: String,
    pub written: u8,
}

/// Change CVC response
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeResponse {
    pub success: bool,
    pub delay_seconds: Option<u32>,
}

/// Sign response
#[derive(Debug, Serialize, Deserialize)]
pub struct SignResponse {
    pub signature: String,
    pub pubkey: String,
}

/// Debug/Status response
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugResponse {
    pub card_type: String,
    pub card_ident: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birth_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slots: Option<SlotInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<u32>>,
    pub applet_version: String,
    pub is_testnet: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SlotInfo {
    pub current: u8,
    pub total: u8,
}

/// Helper function to output response based on format
pub fn output_response<T: Serialize>(response: T, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{json}", json = serde_json::to_string_pretty(&response)?);
        }
        OutputFormat::Plain => {
            // For plain output, we'll need custom formatting per response type
            // This will be implemented as needed for each command
            eprintln!("Plain output not yet implemented for this command");
        }
    }
    Ok(())
}

/// Helper to create success response
pub fn success_response<T>(data: T) -> CommandResponse<T> {
    CommandResponse {
        success: true,
        error: None,
        data: Some(data),
    }
}

/// Helper to create error response
pub fn error_response<T>(error: String) -> CommandResponse<T> {
    CommandResponse {
        success: false,
        error: Some(error),
        data: None,
    }
}
