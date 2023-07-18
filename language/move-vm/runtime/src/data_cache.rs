// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::loader::Loader;

use move_binary_format::errors::*;
use move_core_types::{
    account_address::AccountAddress,
    effects::{AccountChangeSet, ChangeSet, Event},
    identifier::Identifier,
    language_storage::{ModuleId, TypeTag},
    resolver::MoveResolver,
    value::MoveTypeLayout,
    vm_status::StatusCode,
};
use move_vm_types::{
    data_store::DataStore,
    loaded_data::runtime_types::Type,
    values::{GlobalValue, GlobalValueEffect, Value},
};
use std::collections::btree_map::BTreeMap;

// mvmt-patch; jack
use serde::{Deserialize, Serialize};
use serde_json;
use rocksdb::{DB};
use std::sync::Arc;

const TX_CACHE_DB_PATH: &str = "tx-cache.db";
// patch-end



// mvmt-patch; jack; +Serialize, Deserialize
#[derive(Serialize, Deserialize)]
pub struct AccountDataCache {
    data_map: BTreeMap<Type, (MoveTypeLayout, GlobalValue)>,
    module_map: BTreeMap<Identifier, Vec<u8>>,
}

impl AccountDataCache {
    fn new() -> Self {
        Self {
            data_map: BTreeMap::new(),
            module_map: BTreeMap::new(),
        }
    }
}

/// Transaction data cache. Keep updates within a transaction so they can all be published at
/// once when the transaction succeeeds.
///
/// It also provides an implementation for the opcodes that refer to storage and gives the
/// proper guarantees of reference lifetime.
///
/// Dirty objects are serialized and returned in make_write_set.
///
/// It is a responsibility of the client to publish changes once the transaction is executed.
///
/// The Move VM takes a `DataStore` in input and this is the default and correct implementation
/// for a data store related to a transaction. Clients should create an instance of this type
/// and pass it to the Move VM.
pub(crate) struct TransactionDataCache<'r, 'l, S> {
    remote: &'r S,
    loader: &'l Loader,
    account_db: Arc<DB>, //BTreeMap<AccountAddress, AccountDataCache>,
    event_data: Vec<(Vec<u8>, u64, Type, MoveTypeLayout, Value)>,
}

// mvmt-patch; jack
struct ParsedIterator<I>
where
    I: Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>>,
{
    inner: I,
}

impl<I> ParsedIterator<I>
where
    I: Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>>,
{
    fn new(inner: I) -> Self {
        ParsedIterator { inner }
    }
}

impl<I> Iterator for ParsedIterator<I>
where
    I: Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>>,
{
    type Item = (Option<AccountAddress>, Option<AccountDataCache>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| {
            let x = v.ok();
            if let Some((key_bytes, value_bytes)) = x {
                    // Parse or deserialize the key and value
                let parsed_key = parse_key(&key_bytes);
                let parsed_value = deserialize_value(&value_bytes);
                (parsed_key, parsed_value)
            } else {
                (None, None)
            }
        })
    }
}

// Example functions for parsing and deserializing
fn parse_key(key_bytes: &[u8]) -> Option<AccountAddress> {
    serde_json::from_slice::<AccountAddress>(&key_bytes).ok()
}

fn deserialize_value(value_bytes: &[u8]) -> Option<AccountDataCache> {
    serde_json::from_slice::<AccountDataCache>(&value_bytes).ok()
}


impl<'r, 'l, S: MoveResolver> TransactionDataCache<'r, 'l, S> {
    /// Create a `TransactionDataCache` with a `RemoteCache` that provides access to data
    /// not updated in the transaction.
    pub(crate) fn new(remote: &'r S, loader: &'l Loader) -> Self {
        TransactionDataCache {
            remote,
            loader,
            account_db: Arc::new(DB::open_default(TX_CACHE_DB_PATH).unwrap()),
            event_data: vec![],
        }
    }
    // mvmt-patch; jack
    fn load_account_data_cache(&self, addr: &AccountAddress) -> Result<Option<AccountDataCache>, PartialVMError> {
        let account_cache = self
            .account_db
            .get_pinned(addr.as_ref())
            .map_err(|err| PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
            .with_message(format!("RocksDB error: {:?}", err)))?;

        if let Some(data) = account_cache {
            Ok(Some(serde_json::from_slice::<AccountDataCache>(&data).map_err(|err| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message(format!("Failed to deserialize AccountDataCache: {:?}", err))
            })?))
        } else {
            Ok(None)
        }
    }

    fn save_account_data_cache(&self, addr: &AccountAddress, cache: &AccountDataCache) -> Result<(), PartialVMError> {
        let data = serde_json::to_vec(cache).map_err(|err| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(format!("Failed to serialize AccountDataCache: {:?}", err))
        })?;

        self.account_db.put(addr.as_ref(), &data).map_err(|err| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(format!("RocksDB error: {:?}", err))
        })?;
        Ok(())
    }

    /// Make a write set from the updated (dirty, deleted) global resources along with
    /// published modules.
    ///
    /// Gives all proper guarantees on lifetime of global data as well.
    pub(crate) fn into_effects(self) -> PartialVMResult<(ChangeSet, Vec<Event>)> {
        let mut change_set = ChangeSet::new();
        // mvmt-patch; jack
        let parsed_iter = ParsedIterator::new(self.account_db.iterator(rocksdb::IteratorMode::Start));
        for (nullable_addr, nullable_account_data_cache) in parsed_iter {
            if nullable_addr.is_none() || nullable_account_data_cache.is_none() {
                return Err(PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR));
            }
            let addr = nullable_addr.unwrap();
            let account_data_cache = nullable_account_data_cache.unwrap();

            let mut modules = BTreeMap::new();
            for (module_name, module_blob) in account_data_cache.module_map {
                modules.insert(module_name, Some(module_blob));
            }

            let mut resources = BTreeMap::new();
            for (ty, (layout, gv)) in account_data_cache.data_map {
                match gv.into_effect()? {
                    GlobalValueEffect::None => (),
                    GlobalValueEffect::Deleted => {
                        let struct_tag = match self.loader.type_to_type_tag(&ty)? {
                            TypeTag::Struct(struct_tag) => struct_tag,
                            _ => return Err(PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR)),
                        };
                        resources.insert(struct_tag, None);
                    }
                    GlobalValueEffect::Changed(val) => {
                        let struct_tag = match self.loader.type_to_type_tag(&ty)? {
                            TypeTag::Struct(struct_tag) => struct_tag,
                            _ => return Err(PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR)),
                        };
                        let resource_blob = val
                            .simple_serialize(&layout)
                            .ok_or_else(|| PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR))?;
                        resources.insert(struct_tag, Some(resource_blob));
                    }
                }
            }
            change_set.publish_or_overwrite_account_change_set(
                addr,
                AccountChangeSet::from_modules_resources(modules, resources),
            );
        }

        let mut events = vec![];
        for (guid, seq_num, ty, ty_layout, val) in self.event_data {
            let ty_tag = self.loader.type_to_type_tag(&ty)?;
            let blob = val
                .simple_serialize(&ty_layout)
                .ok_or_else(|| PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR))?;
            events.push((guid, seq_num, ty_tag, blob))
        }

        Ok((change_set, events))
    }

    pub(crate) fn num_mutated_accounts(&self, sender: &AccountAddress) -> u64 {
        // The sender's account will always be mutated.
        let mut total_mutated_accounts: u64 = 1;
        // mvmt-patch; jack
        let parsed_iter = ParsedIterator::new(self.account_db.iterator(rocksdb::IteratorMode::Start));
        for (nullable_addr, nullable_entry) in parsed_iter {
            if nullable_addr.is_some() && nullable_entry.is_some() {
                let addr = nullable_addr.unwrap();
                let entry = nullable_entry.unwrap();
                if addr != *sender && entry.data_map.values().any(|(_, v)| v.is_mutated()) {
                    total_mutated_accounts += 1;
                }
            }
        }
        total_mutated_accounts
    }

    // mvmt-patch; jack
    fn _get_mut_or_insert_with<'a, K, V, F>(map: &'a mut BTreeMap<K, V>, k: &K, gen: F) -> &'a mut V
    where
        F: FnOnce() -> (K, V),
        K: Ord,
    {
        if !map.contains_key(k) {
            let (k, v) = gen();
            map.insert(k, v);
        }
        map.get_mut(k).unwrap()
    }
}

// `DataStore` implementation for the `TransactionDataCache`
impl<'r, 'l, S: MoveResolver> DataStore for TransactionDataCache<'r, 'l, S> {
    // Retrieve data from the local cache or loads it from the remote cache into the local cache.
    // All operations on the global data are based on this API and they all load the data
    // into the cache.
    fn load_resource(
        &mut self,
        addr: AccountAddress,
        ty: &Type,
    ) -> PartialVMResult<&mut GlobalValue> {
        // mvmt-patch; jack
        let mut account_cache = self
            .load_account_data_cache(&addr)?
            .unwrap_or_else(AccountDataCache::new);

        if !account_cache.data_map.contains_key(ty) {
            let ty_tag = match self.loader.type_to_type_tag(ty)? {
                TypeTag::Struct(s_tag) => s_tag,
                _ =>
                // non-struct top-level value; can't happen
                {
                    return Err(PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR))
                }
            };
            let ty_layout = self.loader.type_to_type_layout(ty)?;

            let gv = match self.remote.get_resource(&addr, &ty_tag) {
                Ok(Some(blob)) => {
                    let val = match Value::simple_deserialize(&blob, &ty_layout) {
                        Some(val) => val,
                        None => {
                            let msg =
                                format!("Failed to deserialize resource {} at {}!", ty_tag, addr);
                            return Err(PartialVMError::new(
                                StatusCode::FAILED_TO_DESERIALIZE_RESOURCE,
                            )
                            .with_message(msg));
                        }
                    };

                    GlobalValue::cached(val)?
                }
                Ok(None) => GlobalValue::none(),
                Err(err) => {
                    let msg = format!("Unexpected storage error: {:?}", err);
                    return Err(
                        PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                            .with_message(msg),
                    );
                }
            };

            account_cache.data_map.insert(ty.clone(), (ty_layout, gv));
        }
        
        // mvmt-patch; jack
        // TODO - Not completed part
        /*
        Ok(account_cache
            .data_map
            .get_mut(ty)
            .map(|(_ty_layout, gv)| gv)
            .expect("global value must exist"))*/

        Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message("Return Err Result".to_string()),
        )
    }

    fn load_module(&self, module_id: &ModuleId) -> VMResult<Vec<u8>> {
        if let Some(account_cache) = self.load_account_data_cache(module_id.address()).unwrap() {
            if let Some(blob) = account_cache.module_map.get(module_id.name()) {
                return Ok(blob.clone());
            }
        }
        match self.remote.get_module(module_id) {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err(PartialVMError::new(StatusCode::LINKER_ERROR)
                .with_message(format!("Cannot find {:?} in data cache", module_id))
                .finish(Location::Undefined)),
            Err(err) => {
                let msg = format!("Unexpected storage error: {:?}", err);
                Err(
                    PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                        .with_message(msg)
                        .finish(Location::Undefined),
                )
            }
        }
    }

    // mvmt-patch; jack
    fn publish_module(&mut self, module_id: &ModuleId, blob: Vec<u8>) -> VMResult<()> {
        let mut account_cache =
            self.load_account_data_cache(module_id.address()).unwrap().unwrap_or_else(AccountDataCache::new);

        account_cache
            .module_map
            .insert(module_id.name().to_owned(), blob);

        self.save_account_data_cache(module_id.address(), &account_cache).unwrap();
        Ok(())
    }

    fn exists_module(&self, module_id: &ModuleId) -> VMResult<bool> {
        if let Some(account_cache) = self.load_account_data_cache(module_id.address()).unwrap() {
            if account_cache.module_map.contains_key(module_id.name()) {
                return Ok(true);
            }
        }
        Ok(self
            .remote
            .get_module(module_id)
            .map_err(|_| {
                PartialVMError::new(StatusCode::STORAGE_ERROR).finish(Location::Undefined)
            })?
            .is_some())
    }

    fn emit_event(
        &mut self,
        guid: Vec<u8>,
        seq_num: u64,
        ty: Type,
        val: Value,
    ) -> PartialVMResult<()> {
        let ty_layout = self.loader.type_to_type_layout(&ty)?;
        Ok(self.event_data.push((guid, seq_num, ty, ty_layout, val)))
    }

    fn events(&self) -> &Vec<(Vec<u8>, u64, Type, MoveTypeLayout, Value)> {
        &self.event_data
    }
}
