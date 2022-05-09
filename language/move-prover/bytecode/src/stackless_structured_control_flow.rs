// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::stackifier_topological_sort::StackifierTopologicalSort;
use crate::stackless_bytecode::{Bytecode, Label};
use crate::stackless_control_flow_graph::{BlockContent, BlockId, StacklessControlFlowGraph};
use move_model::ast::TempIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::vec::Vec;
use crate::graph::NaturalLoop;
use std::option::Option;

pub struct StacklessStructuredControlFlow {
    pub top_sort: StackifierTopologicalSort,
}

impl StacklessStructuredControlFlow {
    pub fn new(cfg: &StacklessControlFlowGraph) -> Self {
        let mut topological_sort = StackifierTopologicalSort::new();
        for (block_id, block) in &cfg.blocks {
            for successor_id in &block.successors {
                topological_sort.add_dependency(*block_id, *successor_id);
            }
        }
        Self {
            top_sort: topological_sort,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum StacklessSCGBlockKind {
    Basic, // for non-loop blocks
    Break,
    Continue,
}

#[derive(Debug)]
pub enum StacklessSCG {
    BasicBlock{start_offset: usize, end_offset: usize, kind: StacklessSCGBlockKind},
    IfBlock{cond: TempIndex, if_true: Box<StacklessSCG>, if_false: Box<StacklessSCG>},
    LoopBlock{loop_header: Box<StacklessSCG>, loop_body: Vec<StacklessSCG>}
}

impl StacklessSCG {
    pub fn new(block_id: BlockId, cfg: &StacklessControlFlowGraph, kind: StacklessSCGBlockKind) -> Self {
        if let BlockContent::Basic { lower, upper } = cfg.content(block_id) {
            Self::BasicBlock {
                start_offset: *lower as usize,
                end_offset: *upper as usize,
                kind,
            }
        } else {
            panic!();
        }
    }
}

pub fn generate_scg_vec(
    cfg: &StacklessControlFlowGraph,
    code: &[Bytecode],
) -> Vec<StacklessSCG> {
    let prover_graph = cfg.to_prover_graph();
    let mut reduced_cfg = cfg.clone();
    let loop_map = reduced_cfg.reduce_cfg_loop(&prover_graph);
    let mut scf_top_sort = StacklessStructuredControlFlow::new(&reduced_cfg).top_sort;
    let scg_block_map = compute_scg_block_map(&loop_map, &mut scf_top_sort.clone());

    let mut scg_vec = vec![];
    let mut visited_blocks = BTreeSet::new();

    while let Some(blocks) = scf_top_sort.pop() {
        for blk_id in &blocks {
            push_scg(&mut scg_vec, blk_id, &cfg, &mut visited_blocks, code, &loop_map, &scg_block_map);
        }
    }
    println!("the scg vec is {:?}", &scg_vec);
    scg_vec
}

pub fn push_scg(
    scg_vec: &mut Vec<StacklessSCG>,
    blk_id: &BlockId,
    cfg: &StacklessControlFlowGraph,
    visited_blocks: &mut BTreeSet<BlockId>,
    code: &[Bytecode],
    loop_map: &BTreeMap<BlockId, Vec<NaturalLoop<BlockId>>>,
    scg_block_map: &BTreeMap<BlockId, StacklessSCGBlockIndex>,
) {
    if visited_blocks.contains(blk_id) {
        return;
    }
    if let Some(loops) = loop_map.get(&blk_id) {
        push_loop_scg(loops, scg_vec, &cfg, visited_blocks, code, &loop_map, &scg_block_map);
        visited_blocks.insert(*blk_id);
    } else {
        push_non_loop_scg(scg_vec, *blk_id, &cfg, visited_blocks, code, &scg_block_map);
    }
}

pub fn push_loop_scg(
    loops: &Vec<NaturalLoop<BlockId>>,
    scg_vec: &mut Vec<StacklessSCG>,
    // blk_id: BlockId,
    cfg: &StacklessControlFlowGraph,
    visited_blocks: &mut BTreeSet<BlockId>,
    code: &[Bytecode],
    loop_map: &BTreeMap<BlockId, Vec<NaturalLoop<BlockId>>>,
    scg_block_map: &BTreeMap<BlockId, StacklessSCGBlockIndex>,
) {
    let mut loop_scg_vec = vec![];
    let mut loop_header = None;
    
    println!("visited before loop {:?}", &visited_blocks);
    for one_loop in loops {
        for loop_body_blk_id in &one_loop.loop_body {
            if loop_header.is_none() {
                loop_header = Some(one_loop.loop_header);
            }
            if *loop_body_blk_id == one_loop.loop_header {continue;}
            println!("push scg of block {:?} and vec {:?}", loop_body_blk_id, loop_scg_vec);
            push_scg(
                &mut loop_scg_vec,
                loop_body_blk_id,
                &cfg,
                visited_blocks,
                code,
                loop_map,
                scg_block_map,
            );
            println!("visited after loop {:?} and vec {:?}", &visited_blocks, loop_scg_vec);
        }
    }


    scg_vec.push(StacklessSCG::LoopBlock {
        /// Branch is always at the end of a block, here this scg is just a wrapper, so it should be a "basic".
        loop_header: Box::new(StacklessSCG::new(loop_header.unwrap(), &cfg, StacklessSCGBlockKind::Basic)),
        loop_body: loop_scg_vec,
    });
}


pub fn push_non_loop_scg(
    scg_vec: &mut Vec<StacklessSCG>,
    blk_id: BlockId,
    cfg: &StacklessControlFlowGraph,
    visited_blocks: &mut BTreeSet<BlockId>,
    code: &[Bytecode],
    scg_block_map: &BTreeMap<BlockId, StacklessSCGBlockIndex>,
) {
    if visited_blocks.contains(&blk_id) {return;}
    let label_map = compute_label_map(cfg, code);
    let get_block = |l| label_map.get(l).expect("label has corresponding block");
    if let BlockContent::Basic { lower, upper } = cfg.content(blk_id) {
        let mut start = *lower;
        for offs in *lower..*upper + 1 {
            match &code[offs as usize] {
                Bytecode::Branch(_, if_t, if_f, cond) => {
                    scg_vec.push(StacklessSCG::BasicBlock {
                        start_offset: start as usize,
                        end_offset: offs as usize,
                        kind: StacklessSCGBlockKind::Basic,
                    });
                    start = offs;

                    let if_block_id = get_block(if_t);
                    let else_block_id = get_block(if_f);

                    let if_block_kind = get_block_kind(&blk_id, &if_block_id, &scg_block_map);
                    let else_block_kind = get_block_kind(&blk_id, &else_block_id, &scg_block_map);

                    let if_else_scg = StacklessSCG::IfBlock {
                        cond: *cond,
                        if_true: Box::new(StacklessSCG::new(*if_block_id, &cfg, if_block_kind)),
                        if_false: Box::new(StacklessSCG::new(*else_block_id, &cfg, else_block_kind)),
                    };
                    scg_vec.push(if_else_scg);
                    visited_blocks.insert(*if_block_id);
                    visited_blocks.insert(*else_block_id);
                }
                Bytecode::Jump(_, label) => {
                    let dest_blk_id = get_block(label);
                    let block_kind = get_block_kind(&blk_id, dest_blk_id, scg_block_map);
                    scg_vec.push(StacklessSCG::BasicBlock {
                        start_offset: start as usize,
                        end_offset: offs as usize,
                        kind: StacklessSCGBlockKind::Basic,
                    });
                    start = offs;
                }
                _ => {}
            }
        }
        if start != *upper {
            scg_vec.push(StacklessSCG::BasicBlock {
                start_offset: start as usize,
                end_offset: *upper as usize,
                kind: StacklessSCGBlockKind::Basic,
            });
        }
        visited_blocks.insert(blk_id);
    }
}

fn get_block_kind(src_block: &BlockId, dest_block: &BlockId, scg_block_map: &BTreeMap<BlockId, StacklessSCGBlockIndex>) -> StacklessSCGBlockKind {
        let src_index = scg_block_map.get(&src_block).unwrap();
        let dest_index = scg_block_map.get(&dest_block).unwrap();
        match src_index {
            StacklessSCGBlockIndex::LoopBody{index: _, header} => {
                if let Some(header_index) = scg_block_map.get(&header) {
                    match header_index {
                        StacklessSCGBlockIndex::LoopHeader{start_index, end_index, body: _} => {
                            match dest_index {
                                StacklessSCGBlockIndex::Basic{index} => {
                                    if *index == *start_index {
                                        return StacklessSCGBlockKind::Continue;
                                    } else if *index >= *end_index {
                                        // Break to a certain loop point is not supported.
                                        return StacklessSCGBlockKind::Break;
                                    } else if *index < *start_index {
                                        panic!("A go-to towards lines before loop is detected!");
                                    }
                                }
                                StacklessSCGBlockIndex::LoopBody{index, header: _} => {
                                    if *index == *start_index {
                                        return StacklessSCGBlockKind::Continue;
                                    } else if *index >= *end_index {
                                        // Break to a certain loop point is not supported.
                                        return StacklessSCGBlockKind::Break;
                                    } else if *index < *start_index {
                                        panic!("A go-to towards lines before loop is detected!");
                                    }
                                },
                                StacklessSCGBlockIndex::LoopHeader{start_index: _, end_index: _, body: _} => {
                                    return StacklessSCGBlockKind::Continue;
                                }
                            }
                        }
                        _ => {panic!("Header label is of wrong type!")}
                    }
                } else {
                    panic!("Cannot find header label index!");
                }
            }
            _ => {
                return StacklessSCGBlockKind::Basic;
            }
        }
    StacklessSCGBlockKind::Basic
}

/// Compute a map from labels to block ids which those labels start.
pub fn compute_label_map(
    cfg: &StacklessControlFlowGraph,
    code: &[Bytecode],
) -> BTreeMap<Label, BlockId> {
    let mut map = BTreeMap::new();
    for id in cfg.blocks() {
        if let Some(label) = get_label(id, cfg, code) {
            map.insert(label, id);
        }
    }
    map
}

fn get_label(
    id: BlockId, 
    cfg: &StacklessControlFlowGraph,
    code: &[Bytecode]
) -> Option<Label> {
        if let BlockContent::Basic { lower, .. } = cfg.content(id) {
            if let Bytecode::Label(_, label) = &code[*lower as usize] {
                return Some(*label);
            }
        }
        None
    }

#[derive(Debug)]
pub enum StacklessSCGBlockIndex {
    Basic {index: usize},
    LoopHeader {start_index: usize, end_index: usize, body: BTreeSet<BlockId>},
    LoopBody {index: usize, header: BlockId},
}

pub fn compute_scg_block_map(
    loop_map: &BTreeMap<BlockId, Vec<NaturalLoop<BlockId>>>,
    top_sort: &mut StackifierTopologicalSort,
) -> BTreeMap<BlockId, StacklessSCGBlockIndex> {
    let mut scg_block_map = BTreeMap::new();
    let mut scg_block_index = 0;
    let mut visited_blocks = BTreeSet::new();
    while let Some(blocks) = top_sort.pop() {
        for blk_id in &blocks {
            if visited_blocks.contains(blk_id) {continue;}
            if let Some(loops) = loop_map.get(&blk_id) {
                let mut loop_body = BTreeSet::new();
                let mut loop_header = None;
                let loop_header_start_index = scg_block_index;
                for one_loop in loops {
                    for loop_body_blk_id in &one_loop.loop_body {
                        if loop_header.is_none() {
                            loop_header = Some(one_loop.loop_header);
                        }
                        if *loop_body_blk_id == one_loop.loop_header {continue;}
                        let loop_body_label_index = StacklessSCGBlockIndex::LoopBody{index: scg_block_index, header: one_loop.loop_header};
                        scg_block_map.insert(*loop_body_blk_id, loop_body_label_index);

                        loop_body.insert(*loop_body_blk_id);
                        visited_blocks.insert(*loop_body_blk_id);
                        scg_block_index += 1;
                    }
                }
                let loop_header_label_index = StacklessSCGBlockIndex::LoopHeader{
                    start_index: loop_header_start_index,
                    end_index: scg_block_index,
                    body: loop_body,
                };
                scg_block_map.insert(loop_header.unwrap(), loop_header_label_index);
                visited_blocks.insert(loop_header.unwrap());
                scg_block_index += 1;
            } else {
                    scg_block_map.insert(*blk_id, StacklessSCGBlockIndex::Basic{index: scg_block_index});
                    visited_blocks.insert(*blk_id);
                    scg_block_index += 1;
            }
        }
    }
    scg_block_map
}