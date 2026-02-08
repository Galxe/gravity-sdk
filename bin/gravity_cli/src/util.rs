use alloy_primitives::U256;
use std::str::FromStr;

/// Helper function: format ether amount from wei to ETH string
pub fn format_ether(wei: U256) -> String {
    let wei_str = wei.to_string();
    let len = wei_str.len();
    if len <= 18 {
        format!("0.{}", "0".repeat(18 - len) + &wei_str)
    } else {
        let (integer, decimal) = wei_str.split_at(len - 18);
        format!("{}.{}", integer, decimal.trim_end_matches('0').trim_end_matches('.'))
    }
}

/// Helper function: parse ether amount from ETH string to wei
pub fn parse_ether(eth_amount: &str) -> Result<U256, anyhow::Error> {
    const DECIMALS: usize = 18; // 1 Ether = 10^18 Wei

    let parts: Vec<&str> = eth_amount.split('.').collect();

    // Check if there is a decimal point
    if parts.len() == 1 {
        // If integer, append 18 zeros directly
        let s = format!("{}{}", parts[0], "0".repeat(DECIMALS));
        return U256::from_str(&s).map_err(|e| anyhow::anyhow!("Failed to parse ether: {e}"));
    }

    if parts.len() > 2 {
        // Multiple decimal points are invalid input
        return Err(anyhow::anyhow!("Invalid ether amount: {eth_amount}"));
    }

    let integer_part = parts[0];
    let fractional_part = parts[1];

    // Check if fractional part length exceeds 18 digits
    if fractional_part.len() > DECIMALS {
        // Exceeding 18-digit precision is considered invalid or overflow
        return Err(anyhow::anyhow!("Invalid ether amount: {eth_amount}"));
    }

    // Calculate the number of padding zeros needed
    let padding_zeros = DECIMALS - fractional_part.len();

    // Construct final Wei string: [integer part][fractional part][padding zeros]
    let wei_str = format!("{}{}{}", integer_part, fractional_part, "0".repeat(padding_zeros));

    U256::from_str(&wei_str).map_err(|e| anyhow::anyhow!("Failed to parse ether: {e}"))
}
