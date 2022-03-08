// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::language_storage::{ModuleId, StructTag, TypeTag};
use bcs::test_helpers::assert_canonical_encode_decode;
use proptest::prelude::*;
use crate::account_address::AccountAddress;
use crate::identifier::{Identifier, IdentStr};

proptest! {
    #[test]
    fn test_module_id_canonical_roundtrip(module_id in any::<ModuleId>()) {
        assert_canonical_encode_decode(module_id);
    }
}

#[test]
fn test_type_tag_deserialize_case_insensitive() {
    let org_struct_tag = StructTag{
        address: AccountAddress::ONE,
        module: Identifier::from(IdentStr::new("TestModule").unwrap()),
        name: Identifier::from(IdentStr::new("TestStruct").unwrap()),
        type_params: vec![TypeTag::U8, TypeTag::U64, TypeTag::U128, TypeTag::Bool, TypeTag::Address,  TypeTag::Signer]
    };

    let upper_case_json = r#"
    {"address":"0x00000000000000000000000000000001","module":"TestModule","name":"TestStruct","type_params":["U8","U64","U128","Bool","Address","Signer"]}
    "#;
    let upper_case_decoded = serde_json::from_str(&upper_case_json).unwrap();
    assert_eq!(org_struct_tag, upper_case_decoded);

    let lower_case_json = r#"
    {"address":"0x00000000000000000000000000000001","module":"TestModule","name":"TestStruct","type_args":["u8","u64","u128","bool","address","signer"]}
    "#;
    let lower_case_decoded = serde_json::from_str(&lower_case_json).unwrap();
    assert_eq!(org_struct_tag, lower_case_decoded);
}
