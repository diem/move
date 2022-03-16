mod addresses;

use crate::addresses::{bech32_into_address, ss58_to_address, HRP};
use anyhow::{Context, Result};
use move_core_types::account_address::AccountAddress;

pub fn parse_address(addr: &str) -> Result<AccountAddress> {
    if addr.starts_with("0x") {
        let max_hex_len = AccountAddress::LENGTH * 2 + 2;
        if addr.len() > max_hex_len {
            return Err(anyhow::anyhow!(
                "Unable to parse AccountAddress. Maximum address length is {}.  Actual {}",
                max_hex_len,
                addr
            ));
        }

        AccountAddress::from_hex_literal(addr).map_err(|err| err.into())
    } else if cfg!(feature = "bech32_addr") && addr.starts_with(HRP) {
        bech32_into_address(addr)
    } else {
        ss58_to_address(addr)
            .with_context(|| format!("Address {:?} is not a valid diem/pont address", addr))
    }
}
