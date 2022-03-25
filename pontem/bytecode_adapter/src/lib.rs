#[macro_use]
extern crate anyhow;

use anyhow::{ensure, Result};
use move_core_types::account_address::AccountAddress;

use crate::adapt::AddressAdaptation;

mod adapt;
mod context;
mod mutator;

pub const PONTEM_LENGTH: usize = 32;

pub enum AddressType {
    Bech32 = 20,
    Aptos = 16,
}

pub fn adapt_to_pontem(bytes: &mut Vec<u8>, address_type: AddressType) -> Result<()> {
    let adapt = AddressAdaptation::new(address_type as usize, PONTEM_LENGTH);
    adapt.make(bytes)
}

pub fn adapt_from_pontem(bytes: &mut Vec<u8>, address_type: AddressType) -> Result<()> {
    let adapt = AddressAdaptation::new(PONTEM_LENGTH, address_type as usize);
    adapt.make(bytes)
}

pub fn adapt_address_to_target_type(address: AccountAddress, address_type: AddressType) -> Vec<u8> {
    let buffer = address.into_bytes();
    match address_type {
        AddressType::Bech32 => buffer[12..].to_vec(),
        AddressType::Aptos => buffer[16..].to_vec(),
    }
}

pub fn adapt_address_to_pontem(address: &[u8], address_type: AddressType) -> Result<AccountAddress> {
    let buffer = match address_type {
        AddressType::Bech32 => {
            ensure!(
                address.len() == AddressType::Bech32 as usize,
                "Dfninance address must be 20 bytes long."
            );
            let mut buffer = [0; 32];
            buffer[12..].copy_from_slice(address);
            buffer
        }
        AddressType::Aptos => {
            ensure!(
                address.len() == AddressType::Aptos as usize,
                "Diem address must be 16 bytes long."
            );
            let mut buffer = [0; 32];
            buffer[16..].copy_from_slice(address);
            buffer
        }
    };

    Ok(AccountAddress::new(buffer))
}

#[cfg(test)]
mod test {
    use move_core_types::account_address::AccountAddress;

    use crate::{adapt_address_to_pontem, adapt_address_to_target_type, AddressType};

    #[test]
    fn test_address_adaptation() {
        let address = AccountAddress::random();
        let dfi_address = adapt_address_to_target_type(address, AddressType::Bech32);
        assert_eq!(dfi_address.len(), AddressType::Bech32 as usize);

        let diem_address = adapt_address_to_target_type(address, AddressType::Aptos);
        assert_eq!(diem_address.len(), AddressType::Aptos as usize);

        assert_eq!(
            adapt_address_to_target_type(
                adapt_address_to_pontem(&dfi_address, AddressType::Bech32).unwrap(),
                AddressType::Bech32
            ),
            dfi_address
        );
        assert_eq!(&address.to_vec()[12..], &dfi_address[..]);

        assert_eq!(
            adapt_address_to_target_type(
                adapt_address_to_pontem(&diem_address, AddressType::Aptos).unwrap(),
                AddressType::Aptos
            ),
            diem_address
        );
        assert_eq!(&address.to_vec()[16..], &diem_address[..]);
    }
}
