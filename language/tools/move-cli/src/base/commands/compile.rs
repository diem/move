// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use anyhow::Result;
use move_compiler::{
    self,
    shared::{Flags, NumericalAddress},
    Compiler,
};

/// Compile the user modules in `sources` against the dependencies in `interface_files`, placing
/// the resulting binaries in `output_dir`.
pub fn compile(
    interface_files: Vec<String>,
    output_dir: &str,
    sources_shadow_deps: bool,
    sources: Vec<String>,
    named_address_mapping: BTreeMap<String, NumericalAddress>,
    emit_source_map: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("Compiling Move files...");
    }
    let flags = Flags::empty().set_sources_shadow_deps(sources_shadow_deps);
    let (files, compiled_units) = Compiler::new(
        vec![(sources, named_address_mapping.clone())],
        vec![(interface_files, named_address_mapping)],
    )
    .set_flags(flags.clone())
    .build_and_report()?;
    move_compiler::output_compiled_units(emit_source_map, files, compiled_units, output_dir, flags)
}
