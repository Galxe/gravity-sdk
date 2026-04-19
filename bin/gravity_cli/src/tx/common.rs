use alloy_primitives::{Bytes, U256};

/// Standard Solidity revert selectors.
const ERROR_STRING_SELECTOR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0]; // Error(string)
const PANIC_UINT_SELECTOR: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71]; // Panic(uint256)

/// Human-readable decoding of revert bytes returned by eth_call / debug_traceCall.
/// Handles the two standard selectors and falls back to hex for unknown formats.
pub fn decode_revert(data: &[u8]) -> String {
    if data.is_empty() {
        return "reverted (no data)".to_string();
    }
    if data.len() < 4 {
        return format!("raw: 0x{}", hex::encode(data));
    }

    let selector: [u8; 4] = data[0..4].try_into().unwrap();
    let payload = &data[4..];

    if selector == ERROR_STRING_SELECTOR {
        if let Some(msg) = decode_error_string(payload) {
            return format!("Error(\"{msg}\")");
        }
    } else if selector == PANIC_UINT_SELECTOR {
        if payload.len() >= 32 {
            let code = U256::from_be_slice(&payload[0..32]);
            return format!("Panic(0x{code:x})");
        }
    }

    format!("raw: 0x{}", hex::encode(data))
}

/// ABI-decode `Error(string)` payload: [offset:32][length:32][utf8 bytes padded to 32].
fn decode_error_string(payload: &[u8]) -> Option<String> {
    if payload.len() < 64 {
        return None;
    }
    // payload[0..32] is the offset — for a single string arg it's always 0x20,
    // but we don't need to validate it since we use length directly.
    let len = U256::from_be_slice(&payload[32..64]).try_into().ok()?;
    let len: usize = len;
    if payload.len() < 64 + len {
        return None;
    }
    std::str::from_utf8(&payload[64..64 + len]).ok().map(|s| s.to_string())
}

/// Parse a 0x-prefixed hex string returned in JSON-RPC error `data` fields.
pub fn parse_hex_data(raw: &str) -> Option<Bytes> {
    let trimmed = raw.trim().trim_matches('"');
    let no_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if no_prefix.is_empty() {
        return Some(Bytes::new());
    }
    hex::decode(no_prefix).ok().map(Bytes::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_error_string_standard() {
        // keccak("Error(string)")[0..4] = 0x08c379a0
        // Encoded revert for Error("bad stuff"):
        //   08c379a0                                          (selector)
        //   0000...0020                                       (offset = 32)
        //   0000...0009                                       (length = 9)
        //   6261642073747566660000000000000000000000000000000000000000000000 (padded "bad stuff")
        let mut data = hex::decode("08c379a0").unwrap();
        data.extend_from_slice(&[0u8; 31]);
        data.push(0x20);
        data.extend_from_slice(&[0u8; 31]);
        data.push(0x09);
        let mut tail = b"bad stuff".to_vec();
        tail.resize(32, 0);
        data.extend_from_slice(&tail);

        assert_eq!(decode_revert(&data), r#"Error("bad stuff")"#);
    }

    #[test]
    fn decode_panic_arithmetic_overflow() {
        // Panic(uint256) with code 0x11 (arithmetic overflow)
        let mut data = hex::decode("4e487b71").unwrap();
        let mut payload = [0u8; 32];
        payload[31] = 0x11;
        data.extend_from_slice(&payload);

        assert_eq!(decode_revert(&data), "Panic(0x11)");
    }

    #[test]
    fn decode_empty_reverts_to_no_data() {
        assert_eq!(decode_revert(&[]), "reverted (no data)");
    }

    #[test]
    fn decode_unknown_selector_falls_back_to_hex() {
        let data = hex::decode("deadbeef1234").unwrap();
        assert_eq!(decode_revert(&data), "raw: 0xdeadbeef1234");
    }

    #[test]
    fn decode_short_payload_falls_back() {
        let data = hex::decode("ab").unwrap();
        assert_eq!(decode_revert(&data), "raw: 0xab");
    }

    #[test]
    fn parse_hex_handles_quoted_and_unquoted() {
        assert_eq!(parse_hex_data("\"0xdead\"").unwrap().as_ref(), &[0xde, 0xad]);
        assert_eq!(parse_hex_data("0xdead").unwrap().as_ref(), &[0xde, 0xad]);
        assert_eq!(parse_hex_data("dead").unwrap().as_ref(), &[0xde, 0xad]);
        assert_eq!(parse_hex_data("0x").unwrap().as_ref(), &[] as &[u8]);
    }
}
