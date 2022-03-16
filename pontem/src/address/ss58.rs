use anyhow::{anyhow, ensure, Result};
use move_core_types::account_address::AccountAddress;
use rust_base58::{base58::FromBase58, ToBase58};

const SS58_PREFIX: &[u8] = b"SS58PRE";
const PUB_KEY_LENGTH: usize = 32;
const CHECK_SUM_LEN: usize = 2;

fn ss58hash(data: &[u8]) -> blake2_rfc::blake2b::Blake2bResult {
    let mut context = blake2_rfc::blake2b::Blake2b::new(64);
    context.update(SS58_PREFIX);
    context.update(data);
    context.finalize()
}

pub fn ss58_to_address(ss58: &str) -> Result<AccountAddress> {
    let bs58 = match ss58.from_base58() {
        Ok(bs58) => bs58,
        Err(err) => return Err(anyhow!("Wrong base58:{}", err)),
    };
    ensure!(
        bs58.len() > PUB_KEY_LENGTH + CHECK_SUM_LEN,
        format!(
            "Address length must be equal or greater than {} bytes",
            PUB_KEY_LENGTH + CHECK_SUM_LEN
        )
    );
    let check_sum = &bs58[bs58.len() - CHECK_SUM_LEN..];
    let address = &bs58[bs58.len() - PUB_KEY_LENGTH - CHECK_SUM_LEN..bs58.len() - CHECK_SUM_LEN];

    if check_sum != &ss58hash(&bs58[0..bs58.len() - CHECK_SUM_LEN]).as_bytes()[0..CHECK_SUM_LEN] {
        return Err(anyhow!("Wrong address checksum"));
    }
    let mut addr = [0; PUB_KEY_LENGTH];
    addr.copy_from_slice(address);
    Ok(AccountAddress::new(addr))
}

pub fn ss58_to_diem(ss58: &str) -> Result<String> {
    Ok(format!("{:#X}", ss58_to_address(ss58)?))
}

#[cfg(test)]
mod test {
    use super::{ss58_to_address, ss58_to_diem, ss58hash, PUB_KEY_LENGTH};

    #[test]
    fn test_ss58_to_diem_1() {
        let polka_address = "gkNW9pAcCHxZrnoVkhLkEQtsLsW5NWTC75cdAdxAMs9LNYCYg";
        let diem_address = ss58_to_diem(polka_address).unwrap();

        assert_eq!(
            hex::decode(&diem_address[2..]).unwrap().len(),
            PUB_KEY_LENGTH
        );

        assert_eq!(
            "0x8EAF04151687736326C9FEA17E25FC5287613693C912909CB226AA4794F26A48",
            diem_address
        );
    }

    #[test]
    fn test_ss58_to_diem() {
        let polka_address = "G7UkJAutjbQyZGRiP8z5bBSBPBJ66JbTKAkFDq3cANwENyX";
        let diem_address = ss58_to_diem(polka_address).unwrap();

        assert_eq!(
            hex::decode(&diem_address[2..]).unwrap().len(),
            PUB_KEY_LENGTH
        );

        assert_eq!(
            "0x9C786090E2598AE884FF9D1F01D6A1A9BAF13A9E61F73633A8928F4D80BF7DFE",
            diem_address
        );
    }

    #[test]
    fn test_ss58hash() {
        let msg = b"hello, world!";
        let hash = ss58hash(msg).as_bytes().to_vec();

        assert_eq!("656facfcf4f90cce9ec9b65c9185ea75346507c67e25133f5809b442487468a674973f9167193e86bee0c706f6766f7edf638ed3e21ad12c2908ea62924af4d7", hex::encode(hash));
    }
}
