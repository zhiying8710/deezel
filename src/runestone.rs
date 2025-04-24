//! Runestone protocol implementation for DIESEL token minting
//!
//! This module provides functionality for creating Runestone transactions
//! with Protostones for DIESEL token minting.

use bdk::bitcoin::{Script as ScriptBuf, Transaction, TxOut};
use bdk::bitcoin::blockdata::script::{Builder, Instruction};
use bdk::bitcoin::blockdata::opcodes;
use log::debug;
use std::convert::TryInto;

/// Maximum size of a script element
const MAX_SCRIPT_ELEMENT_SIZE: usize = 520;

/// Runestone for DIESEL token minting
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Runestone {
    /// Protocol tag and message
    pub protocol: Option<Vec<u128>>,
}

/// Protocol tag for DIESEL token minting
pub mod tag {
    /// Protocol tag
    pub const PROTOCOL: u128 = 0x0d;
}

/// Varint encoding/decoding utilities
pub mod varint {
    use anyhow::{anyhow, Result};

    /// Encode a u128 as a variable-length integer
    pub fn encode(mut value: u128) -> Vec<u8> {
        let mut result = Vec::new();
        
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            
            if value != 0 {
                byte |= 0x80;
            }
            
            result.push(byte);
            
            if value == 0 {
                break;
            }
        }
        
        result
    }
    
    /// Encode a u128 to a vector
    pub fn encode_to_vec(value: u128, vec: &mut Vec<u8>) {
        vec.extend(encode(value));
    }
    
    /// Decode a variable-length integer from bytes
    pub fn decode(bytes: &[u8]) -> Result<(u128, usize)> {
        let mut result: u128 = 0;
        let mut shift = 0;
        let mut i = 0;
        
        loop {
            if i >= bytes.len() {
                return Err(anyhow!("Truncated varint"));
            }
            
            let byte = bytes[i];
            i += 1;
            
            result |= u128::from(byte & 0x7f) << shift;
            
            if byte & 0x80 == 0 {
                break;
            }
            
            shift += 7;
            
            if shift > 127 {
                return Err(anyhow!("Varint too large"));
            }
        }
        
        Ok((result, i))
    }
    
    /// Decode all integers from a payload
    pub fn decode_all(payload: &[u8]) -> Result<Vec<u128>> {
        let mut integers = Vec::new();
        let mut i = 0;
        
        while i < payload.len() {
            let (integer, length) = decode(&payload[i..])?;
            integers.push(integer);
            i += length;
        }
        
        Ok(integers)
    }
}

impl Runestone {
    /// Magic number for Runestone protocol
    pub const MAGIC_NUMBER: bdk::bitcoin::blockdata::opcodes::All = bdk::bitcoin::blockdata::opcodes::all::OP_PUSHNUM_13;
    
    /// Create a new Runestone with the given protocol tag and message
    pub fn new(protocol_tag: u128, message: &[u8]) -> Self {
        let mut protocol = Vec::new();
        protocol.push(protocol_tag);
        
        // Convert message bytes to u128 values
        for byte in message {
            protocol.push(*byte as u128);
        }
        
        Self {
            protocol: Some(protocol),
        }
    }
    
    /// Create a new DIESEL token minting Runestone
    pub fn new_diesel() -> Self {
        // Protocol tag: 1
        // Message cellpack: [2, 0, 77]
        Self::new(1, &[2, 0, 77])
    }
    
    /// Encode the Runestone as a Bitcoin script
    pub fn encipher(&self) -> bdk::bitcoin::ScriptBuf {
        let mut payload = Vec::new();
        
        // Encode protocol tag and message
        if let Some(protostones) = &self.protocol {
            for proto_u128 in protostones {
                varint::encode_to_vec(tag::PROTOCOL, &mut payload);
                varint::encode_to_vec(*proto_u128, &mut payload);
            }
        }
        
        // Create a script manually with OP_RETURN, magic number, and payload
        let mut script_bytes = Vec::new();
        
        // Add OP_RETURN
        script_bytes.push(0x6a); // OP_RETURN opcode
        
        // Add magic number (OP_PUSHNUM_13)
        script_bytes.push(0x5d); // OP_PUSHNUM_13 opcode
        
        // Add payload in chunks
        for chunk in payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
            if chunk.len() <= 75 {
                // Direct push for small chunks
                script_bytes.push(chunk.len() as u8);
                script_bytes.extend_from_slice(chunk);
            } else if chunk.len() <= 255 {
                // OP_PUSHDATA1 for medium chunks
                script_bytes.push(0x4c); // OP_PUSHDATA1
                script_bytes.push(chunk.len() as u8);
                script_bytes.extend_from_slice(chunk);
            } else {
                // OP_PUSHDATA2 for larger chunks
                script_bytes.push(0x4d); // OP_PUSHDATA2
                script_bytes.push((chunk.len() & 0xff) as u8);
                script_bytes.push((chunk.len() >> 8) as u8);
                script_bytes.extend_from_slice(chunk);
            }
        }
        
        // Create a ScriptBuf from the bytes
        bdk::bitcoin::ScriptBuf::from_bytes(script_bytes)
    }
    
    /// Extract a Runestone from a transaction if present
    pub fn extract(transaction: &Transaction) -> Option<Self> {
        // Search transaction outputs for Runestone
        for output in &transaction.output {
            let mut instructions = output.script_pubkey.instructions();
            
            // Check for OP_RETURN
            if instructions.next() != Some(Ok(Instruction::Op(opcodes::all::OP_RETURN))) {
                continue;
            }
            
            // Check for magic number
            if instructions.next() != Some(Ok(Instruction::Op(Runestone::MAGIC_NUMBER))) {
                continue;
            }
            
            // Construct the payload by concatenating remaining data pushes
            let mut payload = Vec::new();
            
            for result in instructions {
                match result {
                    Ok(Instruction::PushBytes(push)) => {
                        payload.extend_from_slice(push.as_bytes());
                    }
                    Ok(Instruction::Op(_)) => {
                        // Invalid opcode in Runestone payload
                        return None;
                    }
                    Err(_) => {
                        // Invalid script in Runestone payload
                        return None;
                    }
                }
            }
            
            // Decode the integers from the payload
            let integers = match varint::decode_all(&payload) {
                Ok(ints) => ints,
                Err(_) => return None,
            };
            
            // Parse the Runestone data
            let mut protocol_data = Vec::new();
            let mut i = 0;
            
            while i < integers.len() {
                let tag = integers[i];
                i += 1;
                
                // Tag 13 is the protocol tag
                if tag == tag::PROTOCOL && i < integers.len() {
                    protocol_data.push(integers[i]);
                    i += 1;
                } else {
                    // Skip other tags and their values
                    if i < integers.len() {
                        i += 1;
                    }
                }
            }
            
            if !protocol_data.is_empty() {
                return Some(Self {
                    protocol: Some(protocol_data),
                });
            }
        }
        
        None
    }
    
    /// Get the protocol tag (first element in protocol)
    pub fn protocol_tag(&self) -> Option<u128> {
        self.protocol.as_ref().and_then(|p| p.first().copied())
    }
    
    /// Get the message bytes (all elements after the first in protocol)
    pub fn message_bytes(&self) -> Option<Vec<u8>> {
        self.protocol.as_ref().map(|p| {
            p.iter()
                .skip(1)
                .map(|&n| n as u8)
                .collect()
        })
    }
    
    /// Check if this is a DIESEL token minting Runestone
    pub fn is_diesel(&self) -> bool {
        if let Some(tag) = self.protocol_tag() {
            if tag == 1 {
                if let Some(message) = self.message_bytes() {
                    return message == [2, 0, 77];
                }
            }
        }
        false
    }
}
