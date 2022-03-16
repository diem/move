use std::fmt::Write;

use anyhow::{anyhow, ensure, Result};
use move_binary_format::compat::AddressType;
use move_core_types::account_address::AccountAddress;

pub static HRP: &str = "wallet";

pub fn bech32_into_address(address: &str) -> Result<AccountAddress> {
    let (_, data_bytes) = bech32::decode(address)?;
    let data = bech32::convert_bits(&data_bytes, 5, 8, true)?;
    if data.len() != AddressType::Dfninance as usize {
        Err(anyhow!(
            "Invalid dfinance address length [{}]. Expected {} bytes.",
            address,
            AddressType::Dfninance as usize
        ))
    } else {
        let mut address_buff = [0u8; AccountAddress::LENGTH];
        address_buff[AccountAddress::LENGTH - AddressType::Dfninance as usize..]
            .copy_from_slice(&data);
        Ok(AccountAddress::new(address_buff))
    }
}

pub fn bech32_into_diem(address: &str) -> Result<String> {
    let (_, data_bytes) = bech32::decode(address)?;
    let data = bech32::convert_bits(&data_bytes, 5, 8, true)?;

    let mut addr = String::with_capacity(data.len() * 2);
    addr.push_str("0x");
    for byte in &data {
        write!(addr, "{:02X}", byte)?;
    }
    Ok(addr)
}

#[cfg(test)]
pub fn diem_into_bech32(diem_address: &str) -> Result<String> {
    ensure!(
        diem_address.starts_with("0x"),
        "Pass address with 0x prefix"
    );
    let data = hex::decode(&diem_address[2..])?;
    let data = bech32::convert_bits(&data, 8, 5, true)?
        .into_iter()
        .map(bech32::u5::try_from_u8)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(bech32::encode(HRP, data)?)
}

#[cfg(test)]
mod tests {
    use crate::address::bech32::bech32_into_address;
    use move_core_types::account_address::AccountAddress;

    #[test]
    pub fn test_bech32_into_address() {
        assert_eq!(
            bech32_into_address("wallet1me0cdn52672y7feddy7tgcj6j4dkzq2su745vh").unwrap(),
            AccountAddress::from_hex(
                "000000000000000000000000DE5F86CE8AD7944F272D693CB4625A955B610150"
            )
            .unwrap()
        )
    }
}
