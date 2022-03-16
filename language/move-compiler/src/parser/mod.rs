// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

pub mod lexer;
pub mod syntax;

pub mod ast;
pub mod comments;
pub mod keywords;
pub(crate) mod merge_spec_modules;
pub(crate) mod sources_shadow_deps;

use crate::{
    diagnostics::{codes::Severity, Diagnostics, FilesSourceText},
    parser,
    parser::syntax::parse_file_string,
    shared::{AddressScopedFileIndexed, CompilationEnv, NamedAddressMapIndex, NamedAddressMaps},
};
use anyhow::anyhow;
use comments::*;
use move_command_line_common::files::{find_move_filenames, FileHash};
use move_symbol_pool::Symbol;
use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::Read,
};

pub fn parse_program(
    compilation_env: &mut CompilationEnv,
    named_address_maps: NamedAddressMaps,
    targets: Vec<AddressScopedFileIndexed>,
    deps: Vec<AddressScopedFileIndexed>,
) -> anyhow::Result<(
    FilesSourceText,
    Result<(parser::ast::Program, CommentMap), Diagnostics>,
)> {
    fn find_move_filenames_with_address_mapping(
        paths_with_mapping: Vec<AddressScopedFileIndexed>,
    ) -> anyhow::Result<Vec<(Symbol, NamedAddressMapIndex)>> {
        let mut res = vec![];
        for (paths, idx) in paths_with_mapping {
            res.extend(
                find_move_filenames(&[paths], true)?
                    .into_iter()
                    .map(|s| (Symbol::from(s), idx)),
            );
        }
        Ok(res)
    }

    let targets = find_move_filenames_with_address_mapping(targets)?;
    let mut deps = find_move_filenames_with_address_mapping(deps)?;
    ensure_targets_deps_dont_intersect(compilation_env, &targets, &mut deps)?;
    let mut files: FilesSourceText = HashMap::new();
    let mut source_definitions = Vec::new();
    let mut source_comments = CommentMap::new();
    let mut lib_definitions = Vec::new();
    let mut diags: Diagnostics = Diagnostics::new();

    for (fname, address_map_idx) in targets {
        let (defs, comments, ds, file_hash) = parse_file(compilation_env, &mut files, fname)?;
        source_definitions.extend(defs.into_iter().map(|d| (address_map_idx, d)));
        source_comments.insert(file_hash, comments);
        diags.extend(ds);
    }

    for (fname, address_map_idx) in deps {
        let (defs, _, ds, _) = parse_file(compilation_env, &mut files, fname)?;
        lib_definitions.extend(defs.into_iter().map(|d| (address_map_idx, d)));
        diags.extend(ds);
    }

    // TODO fix this so it works likes other passes and the handling of errors is done outside of
    // this function
    let env_result = compilation_env.check_diags_at_or_above_severity(Severity::BlockingError);
    if let Err(env_diags) = env_result {
        diags.extend(env_diags)
    }
    let res = if diags.is_empty() {
        let pprog = parser::ast::Program {
            named_address_maps,
            source_definitions,
            lib_definitions,
        };
        Ok((pprog, source_comments))
    } else {
        Err(diags)
    };
    Ok((files, res))
}

pub fn ensure_targets_deps_dont_intersect(
    compilation_env: &CompilationEnv,
    targets: &[(Symbol, NamedAddressMapIndex)],
    deps: &mut Vec<(Symbol, NamedAddressMapIndex)>,
) -> anyhow::Result<()> {
    /// Canonicalize a file path.
    fn canonicalize(path: &Symbol) -> String {
        let p = path.as_str();
        match std::fs::canonicalize(p) {
            Ok(s) => s.to_string_lossy().to_string(),
            Err(_) => p.to_owned(),
        }
    }
    let target_set = targets
        .iter()
        .map(|(s, _)| canonicalize(s))
        .collect::<BTreeSet<_>>();
    let dep_set = deps
        .iter()
        .map(|(s, _)| canonicalize(s))
        .collect::<BTreeSet<_>>();
    let intersection = target_set.intersection(&dep_set).collect::<Vec<_>>();
    if intersection.is_empty() {
        return Ok(());
    }
    if compilation_env.flags().sources_shadow_deps() {
        deps.retain(|(fname, _)| !intersection.contains(&&canonicalize(fname)));
        return Ok(());
    }
    let all_files = intersection
        .into_iter()
        .map(|s| format!("    {}", s))
        .collect::<Vec<_>>()
        .join("\n");
    Err(anyhow!(
        "The following files were marked as both targets and dependencies:\n{}",
        all_files
    ))
}

pub fn parse_file(
    compilation_env: &mut CompilationEnv,
    files: &mut FilesSourceText,
    fname: Symbol,
) -> anyhow::Result<(
    Vec<parser::ast::Definition>,
    MatchedFileCommentMap,
    Diagnostics,
    FileHash,
)> {
    let mut diags = Diagnostics::new();
    let mut f = File::open(fname.as_str())
        .map_err(|err| std::io::Error::new(err.kind(), format!("{}: {}", err, fname)))?;
    let mut source_buffer = String::new();
    f.read_to_string(&mut source_buffer)?;
    let file_hash = FileHash::new(&source_buffer);
    let buffer = match verify_string(file_hash, &source_buffer) {
        Err(ds) => {
            diags.extend(ds);
            files.insert(file_hash, (fname, source_buffer));
            return Ok((vec![], MatchedFileCommentMap::new(), diags, file_hash));
        }
        Ok(()) => &source_buffer,
    };
    let (defs, comments) = match parse_file_string(compilation_env, file_hash, buffer) {
        Ok(defs_and_comments) => defs_and_comments,
        Err(ds) => {
            diags.extend(ds);
            (vec![], MatchedFileCommentMap::new())
        }
    };
    files.insert(file_hash, (fname, source_buffer));
    Ok((defs, comments, diags, file_hash))
}
