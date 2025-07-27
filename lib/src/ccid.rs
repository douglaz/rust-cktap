use thiserror::Error;

/// CCID (Chip Card Interface Device) protocol implementation
/// Based on USB-IF CCID specification v1.1
/// CCID message header (10 bytes)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CcidHeader {
    pub message_type: u8,
    pub length: u32, // Little-endian
    pub slot: u8,
    pub sequence: u8,
    pub reserved: [u8; 3],
}

impl CcidHeader {
    pub fn new(message_type: MessageType, length: u32, slot: u8, sequence: u8) -> Self {
        Self {
            message_type: message_type as u8,
            length: length.to_le(),
            slot,
            sequence,
            reserved: [0; 3],
        }
    }

    pub fn to_bytes(&self) -> [u8; 10] {
        unsafe { std::mem::transmute_copy(self) }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CcidError> {
        if bytes.len() < 10 {
            return Err(CcidError::InvalidHeader);
        }

        Ok(Self {
            message_type: bytes[0],
            length: u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
            slot: bytes[5],
            sequence: bytes[6],
            reserved: [bytes[7], bytes[8], bytes[9]],
        })
    }
}

/// CCID message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    // PC to RDR (Host to Reader)
    PcToRdrIccPowerOn = 0x62,
    PcToRdrIccPowerOff = 0x63,
    PcToRdrGetSlotStatus = 0x65,
    PcToRdrXfrBlock = 0x6F,
    PcToRdrGetParameters = 0x6C,
    PcToRdrResetParameters = 0x6D,
    PcToRdrSetParameters = 0x61,
    PcToRdrEscape = 0x6B,
    PcToRdrIccClock = 0x6E,
    PcToRdrT0APDU = 0x6A,
    PcToRdrSecure = 0x69,
    PcToRdrMechanical = 0x71,
    PcToRdrAbort = 0x72,
    PcToRdrSetDataRateAndClockFrequency = 0x73,

    // RDR to PC (Reader to Host)
    RdrToPcDataBlock = 0x80,
    RdrToPcSlotStatus = 0x81,
    RdrToPcParameters = 0x82,
    RdrToPcEscape = 0x83,
    RdrToPcDataRateAndClockFrequency = 0x84,
}

impl TryFrom<u8> for MessageType {
    type Error = CcidError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x62 => Ok(Self::PcToRdrIccPowerOn),
            0x63 => Ok(Self::PcToRdrIccPowerOff),
            0x65 => Ok(Self::PcToRdrGetSlotStatus),
            0x6F => Ok(Self::PcToRdrXfrBlock),
            0x6C => Ok(Self::PcToRdrGetParameters),
            0x6D => Ok(Self::PcToRdrResetParameters),
            0x61 => Ok(Self::PcToRdrSetParameters),
            0x6B => Ok(Self::PcToRdrEscape),
            0x6E => Ok(Self::PcToRdrIccClock),
            0x6A => Ok(Self::PcToRdrT0APDU),
            0x69 => Ok(Self::PcToRdrSecure),
            0x71 => Ok(Self::PcToRdrMechanical),
            0x72 => Ok(Self::PcToRdrAbort),
            0x73 => Ok(Self::PcToRdrSetDataRateAndClockFrequency),
            0x80 => Ok(Self::RdrToPcDataBlock),
            0x81 => Ok(Self::RdrToPcSlotStatus),
            0x82 => Ok(Self::RdrToPcParameters),
            0x83 => Ok(Self::RdrToPcEscape),
            0x84 => Ok(Self::RdrToPcDataRateAndClockFrequency),
            _ => Err(CcidError::UnknownMessageType(value)),
        }
    }
}

/// CCID commands
#[derive(Debug, Clone)]
pub struct CcidCommand {
    pub header: CcidHeader,
    pub data: Vec<u8>,
}

impl CcidCommand {
    /// Create a PC_to_RDR_IccPowerOn command
    pub fn icc_power_on(slot: u8, sequence: u8, voltage: VoltageSelection) -> Self {
        let mut header = CcidHeader::new(MessageType::PcToRdrIccPowerOn, 0, slot, sequence);
        header.reserved[0] = voltage as u8; // bPowerSelect

        Self {
            header,
            data: Vec::new(),
        }
    }

    /// Create a PC_to_RDR_XfrBlock command
    pub fn xfr_block(slot: u8, sequence: u8, apdu: Vec<u8>) -> Self {
        let header = CcidHeader::new(
            MessageType::PcToRdrXfrBlock,
            apdu.len() as u32,
            slot,
            sequence,
        );

        Self { header, data: apdu }
    }

    /// Create a PC_to_RDR_GetSlotStatus command
    pub fn get_slot_status(slot: u8, sequence: u8) -> Self {
        let header = CcidHeader::new(MessageType::PcToRdrGetSlotStatus, 0, slot, sequence);

        Self {
            header,
            data: Vec::new(),
        }
    }

    /// Convert command to bytes for transmission
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(10 + self.data.len());
        bytes.extend_from_slice(&self.header.to_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }
}

/// CCID responses
#[derive(Debug, Clone)]
pub struct CcidResponse {
    pub header: CcidHeader,
    pub data: Vec<u8>,
    pub slot_status: SlotStatus,
    pub slot_error: SlotError,
}

impl CcidResponse {
    /// Parse a CCID response from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CcidError> {
        if bytes.len() < 10 {
            return Err(CcidError::InvalidResponse);
        }

        let header = CcidHeader::from_bytes(bytes)?;
        let data_len = header.length as usize;

        if bytes.len() < 10 + data_len {
            return Err(CcidError::InvalidResponse);
        }

        // For all RDR_to_PC messages, byte 7 is bStatus (slot status + error)
        let status_byte = bytes[7];
        let slot_status = SlotStatus::from_bits(status_byte & 0x03)?;
        let slot_error = SlotError::from_bits(status_byte >> 6)?;

        let data = bytes[10..10 + data_len].to_vec();

        Ok(Self {
            header,
            data,
            slot_status,
            slot_error,
        })
    }
}

/// Voltage selection for ICC power on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VoltageSelection {
    Automatic = 0x00,
    Voltage5V = 0x01,
    Voltage3V = 0x02,
    Voltage1_8V = 0x03,
}

/// Slot status bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotStatus {
    ActiveICC = 0,
    InactiveICC = 1,
    NoICCPresent = 2,
}

impl SlotStatus {
    fn from_bits(bits: u8) -> Result<Self, CcidError> {
        match bits & 0x03 {
            0 => Ok(Self::ActiveICC),
            1 => Ok(Self::InactiveICC),
            2 => Ok(Self::NoICCPresent),
            _ => Err(CcidError::InvalidSlotStatus),
        }
    }
}

/// Slot error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotError {
    NoError = 0,
    CommandError = 1,
    MoreTime = 2,
    HardwareError = 3,
}

impl SlotError {
    fn from_bits(bits: u8) -> Result<Self, CcidError> {
        match bits & 0x03 {
            0 => Ok(Self::NoError),
            1 => Ok(Self::CommandError),
            2 => Ok(Self::MoreTime),
            3 => Ok(Self::HardwareError),
            _ => Err(CcidError::InvalidSlotError),
        }
    }
}

/// CCID specific errors
#[derive(Debug, Clone, Error)]
pub enum CcidError {
    #[error("Invalid CCID header")]
    InvalidHeader,

    #[error("Invalid CCID response")]
    InvalidResponse,

    #[error("Unknown message type: {0:#x}")]
    UnknownMessageType(u8),

    #[error("Invalid slot status")]
    InvalidSlotStatus,

    #[error("Invalid slot error")]
    InvalidSlotError,

    #[error("ICC mute (no response)")]
    IccMute,

    #[error("ICC error: {0}")]
    IccError(String),

    #[error("Command aborted")]
    CommandAborted,

    #[error("Time extension requested")]
    TimeExtension,

    #[error("Hardware error")]
    HardwareError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<CcidHeader>(), 10);
    }

    #[test]
    fn test_header_conversion() {
        let header = CcidHeader::new(MessageType::PcToRdrXfrBlock, 5, 0, 1);
        let bytes = header.to_bytes();
        let parsed = CcidHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.message_type, parsed.message_type);
        // Copy fields to avoid unaligned reference to packed field
        let header_length = header.length;
        let parsed_length = parsed.length;
        assert_eq!(header_length, parsed_length);
        let header_slot = header.slot;
        let parsed_slot = parsed.slot;
        assert_eq!(header_slot, parsed_slot);
        let header_sequence = header.sequence;
        let parsed_sequence = parsed.sequence;
        assert_eq!(header_sequence, parsed_sequence);
    }

    #[test]
    fn test_command_creation() {
        let apdu = vec![0x00, 0xA4, 0x04, 0x00];
        let cmd = CcidCommand::xfr_block(0, 1, apdu.clone());

        // Copy packed fields to local variables to avoid unaligned access
        let message_type = cmd.header.message_type;
        let length = cmd.header.length;

        assert_eq!(message_type, MessageType::PcToRdrXfrBlock as u8);
        assert_eq!(length, 4);
        assert_eq!(cmd.data, apdu);
    }
}
