mod addresses;

use crate::addresses::{bech32_into_address, ss58_to_address, HRP};
use anyhow::{Context, Result};
use move_core_types::account_address::AccountAddress;

pub fn parse_address(addr: &str) -> Result<AccountAddress> {
    if let Ok(address) = ss58_to_address(addr) {
        // first try ss58 parsing
        Ok(address)
    } else if cfg!(feature = "bech32_addr") && addr.starts_with(HRP) {
        // try bech32 address
        bech32_into_address(addr)
    } else {
        let mut addr = addr.to_string();
        if !addr.starts_with("0x") {
            addr = format!("0x{}", addr);
        }
        // try parsing hex diem/aptos address with optional 0x prefix
        let max_hex_len = AccountAddress::LENGTH * 2 + 2;
        if addr.len() > max_hex_len {
            return Err(anyhow::anyhow!(
                "Unable to parse AccountAddress. Maximum address length is {}.  Actual {}",
                max_hex_len,
                addr
            ));
        }
        AccountAddress::from_hex_literal(&addr)
            .with_context(|| format!("Address {:?} is not a valid diem/pont address", addr))
    }
}
