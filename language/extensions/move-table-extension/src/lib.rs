// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! A crate which extends Move by tables.
//!
//! See [`Table.move`](../sources/Table.move) for language use.
//! See [`README.md`](../README.md) for integration into an adapter.

use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress,
    gas_schedule::{GasAlgebra, GasCarrier, InternalGasUnits},
    value::MoveTypeLayout,
    vm_status::StatusCode,
};
use move_vm_runtime::{
    native_functions,
    native_functions::{NativeContext, NativeFunctionTable},
};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{GlobalValue, GlobalValueEffect, Reference, StructRef, Value},
};
use once_cell::sync::Lazy;
use sha3::{Digest, Sha3_256};
use smallvec::smallvec;
use std::{
    cell::RefCell,
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    convert::TryInto,
    fmt::Display,
};

// ===========================================================================================
// Public Data Structures and Constants

/// The representation of a table handle. This is created from truncating a sha3-256 based
/// hash over a transaction hash provided by the environment and a table creation counter
/// local to the transaction.
#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct TableHandle(pub u128);

impl Display for TableHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "T-{:X}", self.0)
    }
}

/// A table change set.
#[derive(Default, Debug)]
pub struct TableChangeSet {
    pub new_tables: BTreeSet<TableHandle>,
    pub removed_tables: BTreeSet<TableHandle>,
    pub changes: BTreeMap<TableHandle, TableChange>,
}

/// A change of a single table.
#[derive(Debug)]
pub struct TableChange {
    pub entries: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    pub base_size: u64,
    pub size_delta: i64,
}

/// A table resolver which needs to be provided by the environment. This allows to lookup
/// data in remote storage, as well as retrieve cost of table operations.
pub trait TableResolver {
    fn resolve_table_entry(
        &self,
        handle: &TableHandle,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, anyhow::Error>;

    fn table_size(&self, handle: &TableHandle) -> Result<usize, anyhow::Error>;

    fn operation_cost(
        &self,
        op: TableOperation,
        key_size: usize,
        val_size: usize,
    ) -> InternalGasUnits<GasCarrier>;
}

struct UnsafeStaticTableResolver {
    inner: *const dyn TableResolver,
}

impl UnsafeStaticTableResolver {
    fn unsafe_new(resolver: &dyn TableResolver) -> Self {
        unsafe {
            Self {
                inner: std::mem::transmute::<&dyn TableResolver, *const dyn TableResolver>(
                    resolver,
                ),
            }
        }
    }

    fn unsafe_get(&self) -> &dyn TableResolver {
        unsafe { &*self.inner as &dyn TableResolver }
    }
}

impl TableResolver for UnsafeStaticTableResolver {
    fn resolve_table_entry(
        &self,
        handle: &TableHandle,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, anyhow::Error> {
        self.unsafe_get().resolve_table_entry(handle, key)
    }

    fn table_size(&self, handle: &TableHandle) -> Result<usize, anyhow::Error> {
        self.unsafe_get().table_size(handle)
    }

    fn operation_cost(
        &self,
        op: TableOperation,
        key_size: usize,
        val_size: usize,
    ) -> InternalGasUnits<GasCarrier> {
        self.unsafe_get().operation_cost(op, key_size, val_size)
    }
}

/// A table operation, for supporting cost calculation.
pub enum TableOperation {
    NewHandle,
    Destroy,
    Insert,
    Borrow,
    Length,
    Remove,
    Contains,
}

/// The native table context extension. This needs to be attached to the NativeContextExtensions
/// value which is passed into session functions, so its accessible from natives of this
/// extension.
pub struct NativeTableContext {
    resolver: UnsafeStaticTableResolver,
    txn_hash: u128,
    table_data: RefCell<TableData>,
}

pub static ALREADY_EXISTS: Lazy<u64> = Lazy::new(|| unique_sub_status_code(0));
pub static NOT_FOUND: Lazy<u64> = Lazy::new(|| unique_sub_status_code(1));
pub static NOT_EMPTY: Lazy<u64> = Lazy::new(|| unique_sub_status_code(2));

// ===========================================================================================
// Private Data Structures and Constants

/// A structure representing mutable data of the NativeTableContext. This is in a RefCell
/// of the overall context so we can mutate while still accessing the overall context.
#[derive(Default)]
struct TableData {
    new_tables: BTreeSet<TableHandle>,
    removed_tables: BTreeSet<TableHandle>,
    tables: BTreeMap<TableHandle, Table>,
}

/// A structure representing a single table.
struct Table {
    handle: TableHandle,
    key_layout: MoveTypeLayout,
    value_layout: MoveTypeLayout,
    content: BTreeMap<Vec<u8>, GlobalValue>,
    base_size: u64,
    size_delta: i64, // The sum of added and removed entries
}

/// The field index of the `handle` field in the `Table` Move struct.
const HANDLE_FIELD_INDEX: usize = 0;

// =========================================================================================
// Implementation of Native Table Context

impl NativeTableContext {
    /// Create a new instance of a native table context. This must be passed in via an
    /// extension into VM session functions.
    pub fn new(txn_hash: u128, resolver: &dyn TableResolver) -> Self {
        Self {
            resolver: UnsafeStaticTableResolver::unsafe_new(resolver),
            txn_hash,
            table_data: Default::default(),
        }
    }

    /// Computes the change set from a NativeTableContext.
    pub fn into_change_set(self) -> PartialVMResult<TableChangeSet> {
        let NativeTableContext { table_data, .. } = self;
        let TableData {
            new_tables,
            removed_tables,
            tables,
        } = table_data.into_inner();
        let mut changes = BTreeMap::new();
        for (handle, table) in tables {
            let Table {
                value_layout,
                content,
                base_size,
                size_delta,
                ..
            } = table;
            let mut entries = BTreeMap::new();
            for (key, gv) in content {
                match gv.into_effect()? {
                    GlobalValueEffect::Deleted => {
                        entries.insert(key, None);
                    }
                    GlobalValueEffect::Changed(new_val) => {
                        let new_bytes = serialize(&value_layout, &new_val)?;
                        entries.insert(key, Some(new_bytes));
                    }
                    _ => {}
                }
            }
            changes.insert(
                handle,
                TableChange {
                    entries,
                    base_size,
                    size_delta,
                },
            );
        }
        Ok(TableChangeSet {
            new_tables,
            removed_tables,
            changes,
        })
    }
}

impl TableData {
    /// Gets or creates a new table in the TableData. This initializes information about
    /// the table, like the type layout for keys and values.
    fn get_table(
        &mut self,
        context: &NativeContext,
        handle: TableHandle,
        key_ty: &Type,
        value_ty: &Type,
    ) -> PartialVMResult<&mut Table> {
        self.get_or_create_table_impl(context, handle, key_ty, value_ty, false)
    }

    fn new_table(
        &mut self,
        context: &NativeContext,
        handle: TableHandle,
        key_ty: &Type,
        value_ty: &Type,
    ) -> PartialVMResult<&mut Table> {
        self.get_or_create_table_impl(context, handle, key_ty, value_ty, true)
    }

    fn get_or_create_table_impl(
        &mut self,
        context: &NativeContext,
        handle: TableHandle,
        key_ty: &Type,
        value_ty: &Type,
        new_table: bool,
    ) -> PartialVMResult<&mut Table> {
        if let Entry::Vacant(e) = self.tables.entry(handle) {
            let table_context = context.extensions().get::<NativeTableContext>();
            let key_layout = get_type_layout(context, key_ty)?;
            let value_layout = get_type_layout(context, value_ty)?;
            let base_size = if new_table {
                if let Ok(table_size) = table_context.resolver.table_size(&handle) {
                    assert_eq!(table_size, 0);
                }
                0
            } else {
                table_context.resolver.table_size(&handle).map_err(|e| {
                    partial_extension_error(format!("table_size() failed with {}", e))
                })? as u64
            };
            let table = Table {
                handle,
                key_layout,
                value_layout,
                base_size,
                size_delta: 0,
                content: Default::default(),
            };
            e.insert(table);
        } else if new_table {
            return Err(partial_abort_error(
                "Table already exists.",
                *ALREADY_EXISTS,
            ));
        }
        Ok(self.tables.get_mut(&handle).unwrap())
    }
}

impl Table {
    /// Inserts a value into a table.
    fn insert(
        &mut self,
        context: &NativeTableContext,
        key: &Value,
        val: Value,
    ) -> PartialVMResult<(usize, usize)> {
        let (gv_opt, _, _) = self.global_value_if_exists(context, key)?;
        if gv_opt.is_some() {
            return Err(partial_abort_error(
                "table entry already occupied",
                *ALREADY_EXISTS,
            ));
        }
        let key_bytes = serialize(&self.key_layout, key)?;
        let key_size = key_bytes.len();
        // Need to serialize for cost computation
        let val_size = serialize(&self.value_layout, &val)?.len();
        self.content
            .entry(key_bytes)
            .or_insert_with(GlobalValue::none)
            .move_to(val)?;
        self.size_delta += 1;
        Ok((key_size, val_size))
    }

    /// Borrows a reference to a table (mutable or immutable).
    fn borrow_global(
        &mut self,
        context: &NativeTableContext,
        key: &Value,
    ) -> PartialVMResult<(Value, usize, usize)> {
        let (gv_opt, key_size, val_size) = self.global_value_if_exists(context, key)?;
        let gv = gv_opt.ok_or_else(|| partial_abort_error("undefined table entry", *NOT_FOUND))?;
        let val = gv.borrow_global()?;
        Ok((val, key_size, val_size))
    }

    /// Removes an entry from a table.
    fn remove(
        &mut self,
        context: &NativeTableContext,
        key: &Value,
    ) -> PartialVMResult<(Value, usize, usize)> {
        let (gv_opt, key_size, val_size) = self.global_value_if_exists(context, key)?;
        let gv = gv_opt.ok_or_else(|| partial_abort_error("undefined table entry", *NOT_FOUND))?;
        let val = gv.move_from()?;
        self.size_delta -= 1;
        Ok((val, key_size, val_size))
    }

    /// Checks whether a key is in the table.
    fn contains(
        &mut self,
        context: &NativeTableContext,
        key: &Value,
    ) -> PartialVMResult<(Value, usize, usize)> {
        let (gv_opt, key_size, val_size) = self.global_value_if_exists(context, key)?;
        Ok((Value::bool(gv_opt.is_some()), key_size, val_size))
    }

    /// Compute the size of a table.
    fn length(&mut self, _context: &NativeTableContext) -> PartialVMResult<(u64, usize, usize)> {
        let effective_size = (self.base_size as i128) + (self.size_delta as i128);
        if effective_size < 0 {
            Err(partial_extension_error("inconsistent table size"))
        } else {
            Ok((effective_size as u64, 0, 0))
        }
    }

    /// Destroys a table.
    fn destroy_empty(&mut self, context: &NativeTableContext) -> PartialVMResult<(usize, usize)> {
        let (len, _, _) = self.length(context)?;
        if len > 0 {
            Err(partial_abort_error(
                "table is not empty and cannot be destroyed",
                *NOT_EMPTY,
            ))
        } else {
            Ok((0, 0))
        }
    }

    /// Gets the global value of an entry in the table. Attempts to retrieve a value from
    /// the resolver if needed. Aborts if the value does not exists. Also returns the size
    /// of the key and value (if a value needs to be fetched from remote) for cost computation.
    fn global_value_if_exists(
        &mut self,
        context: &NativeTableContext,
        key: &Value,
    ) -> PartialVMResult<(Option<&mut GlobalValue>, usize, usize)> {
        let key_bytes = serialize(&self.key_layout, key)?;
        let key_size = key_bytes.len();
        let mut val_size = 0;

        if !self.content.contains_key(&key_bytes) {
            // Try to retrieve a value from the remote resolver.
            let gv = match context
                .resolver
                .resolve_table_entry(&self.handle, &key_bytes)
                .map_err(|err| {
                    partial_extension_error(format!("remote table resolver failure: {}", err))
                })? {
                Some(val_bytes) => {
                    val_size = val_bytes.len();
                    let val = deserialize(&self.value_layout, &val_bytes)?;
                    GlobalValue::cached(val)?
                }
                None => GlobalValue::none(),
            };
            self.content.insert(key_bytes.clone(), gv);
        }

        let gv = self.content.get_mut(&key_bytes).unwrap();
        if gv.exists()? {
            Ok((Some(gv), key_size, val_size))
        } else {
            Ok((None, key_size, val_size))
        }
    }
}

// =========================================================================================
// Native Function Implementations

/// Returns all natives for tables.
pub fn table_natives(table_addr: AccountAddress) -> NativeFunctionTable {
    native_functions::make_table(
        table_addr,
        &[
            ("Table", "new_table_handle", native_new_table_handle),
            ("Table", "add_box", native_add_box),
            ("Table", "length_box", native_length_box),
            ("Table", "borrow_box", native_borrow_box),
            ("Table", "borrow_box_mut", native_borrow_box),
            ("Table", "remove_box", native_remove_box),
            ("Table", "contains_box", native_contains_box),
            ("Table", "destroy_empty_box", native_destroy_empty_box),
            ("Table", "drop_unchecked_box", native_drop_unchecked_box),
        ],
    )
}

fn native_new_table_handle(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.is_empty());

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    // Take the transaction hash provided by the environment, combine it with the # of tables
    // produced so far, sha256 this and select 16 bytes from the result. Given the txn hash
    // is unique, this should create a unique and deterministic global id.
    let mut digest = Sha3_256::new();
    Digest::update(&mut digest, table_context.txn_hash.to_be_bytes());
    Digest::update(&mut digest, table_data.new_tables.len().to_be_bytes());
    let bytes: [u8; 16] = digest.finalize()[0..16].try_into().unwrap();
    let id = u128::from_be_bytes(bytes);
    let handle = TableHandle(id);
    assert!(table_data.new_tables.insert(handle));
    table_data.new_table(context, handle, &ty_args[0], &ty_args[2])?;

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::NewHandle, 0, 0),
        smallvec![Value::u128(id)],
    ))
}

fn native_add_box(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 3);

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    let val = args.pop_back().unwrap();
    let key = args
        .pop_back()
        .unwrap()
        .value_as::<Reference>()?
        .read_ref()?;
    let handle = get_table_handle(&pop_arg!(args, StructRef))?;

    let table = table_data.get_table(context, handle, &ty_args[0], &ty_args[2])?;
    let status = table.insert(table_context, &key, val);
    let (key_size, val_size) = status?;

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::Insert, key_size, val_size),
        smallvec![],
    ))
}

fn native_length_box(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 1);

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    let handle = get_table_handle(&pop_arg!(args, StructRef))?;

    let table = table_data.get_table(context, handle, &ty_args[0], &ty_args[2])?;
    let (len, key_size, val_size) = table.length(table_context)?;

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::Length, key_size, val_size),
        smallvec![Value::u64(len)],
    ))
}

fn native_borrow_box(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 2);

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    let key = args
        .pop_back()
        .unwrap()
        .value_as::<Reference>()?
        .read_ref()?;
    let handle = get_table_handle(&pop_arg!(args, StructRef))?;

    let table = table_data.get_table(context, handle, &ty_args[0], &ty_args[2])?;
    let (val, key_size, val_size) = table.borrow_global(table_context, &key)?;

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::Borrow, key_size, val_size),
        smallvec![val],
    ))
}

fn native_contains_box(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 2);

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    let key = args
        .pop_back()
        .unwrap()
        .value_as::<Reference>()?
        .read_ref()?;
    let handle = get_table_handle(&pop_arg!(args, StructRef))?;

    let table = table_data.get_table(context, handle, &ty_args[0], &ty_args[2])?;
    let (val, key_size, val_size) = table.contains(table_context, &key)?;

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::Contains, key_size, val_size),
        smallvec![val],
    ))
}

fn native_remove_box(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 2);

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    let key = args
        .pop_back()
        .unwrap()
        .value_as::<Reference>()?
        .read_ref()?;
    let handle = get_table_handle(&pop_arg!(args, StructRef))?;
    let table = table_data.get_table(context, handle, &ty_args[0], &ty_args[2])?;
    let (val, key_size, val_size) = table.remove(table_context, &key)?;

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::Remove, key_size, val_size),
        smallvec![val],
    ))
}

fn native_destroy_empty_box(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 1);

    let table_context = context.extensions().get::<NativeTableContext>();
    let mut table_data = table_context.table_data.borrow_mut();

    let handle = get_table_handle(&pop_arg!(args, StructRef))?;
    let table = table_data.get_table(context, handle, &ty_args[0], &ty_args[2])?;
    let (key_size, val_size) = table.destroy_empty(table_context)?;

    assert!(table_data.removed_tables.insert(handle));

    Ok(NativeResult::ok(
        table_context
            .resolver
            .operation_cost(TableOperation::Destroy, key_size, val_size),
        smallvec![],
    ))
}

fn native_drop_unchecked_box(
    _context: &mut NativeContext,
    ty_args: Vec<Type>,
    args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 3);
    assert!(args.len() == 1);

    Ok(NativeResult::ok(InternalGasUnits::new(0_u64), smallvec![]))
}

// =========================================================================================
// Helpers

fn get_table_handle(table: &StructRef) -> PartialVMResult<TableHandle> {
    let field_ref = table
        .borrow_field(HANDLE_FIELD_INDEX)?
        .value_as::<Reference>()?;
    field_ref.read_ref()?.value_as::<u128>().map(TableHandle)
}

fn serialize(layout: &MoveTypeLayout, val: &Value) -> PartialVMResult<Vec<u8>> {
    val.simple_serialize(layout)
        .ok_or_else(|| partial_extension_error("cannot serialize table key or value"))
}

fn deserialize(layout: &MoveTypeLayout, bytes: &[u8]) -> PartialVMResult<Value> {
    Value::simple_deserialize(bytes, layout)
        .ok_or_else(|| partial_extension_error("cannot deserialize table key or value"))
}

fn partial_extension_error(msg: impl ToString) -> PartialVMError {
    PartialVMError::new(StatusCode::VM_EXTENSION_ERROR).with_message(msg.to_string())
}

fn partial_abort_error(msg: impl ToString, code: u64) -> PartialVMError {
    PartialVMError::new(StatusCode::ABORTED)
        .with_message(msg.to_string())
        .with_sub_status(code)
}

fn get_type_layout(context: &NativeContext, ty: &Type) -> PartialVMResult<MoveTypeLayout> {
    context
        .type_to_type_layout(ty)?
        .ok_or_else(|| partial_extension_error("cannot determine type layout"))
}

fn unique_sub_status_code(logical_code: u8) -> u64 {
    let mut digest = Sha3_256::new();
    Digest::update(&mut digest, "Extensions::Table");
    Digest::update(&mut digest, logical_code.to_be_bytes());
    let bytes: [u8; 2] = digest.finalize()[0..2].try_into().unwrap();
    u16::from_be_bytes(bytes) as u64
}
