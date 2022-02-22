// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::stackless_control_flow_graph::BlockId;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct StackifierDependency {
    pub num_prec: usize,
    pub succ_set: BTreeSet<BlockId>,
}

#[derive(Clone, Debug)]
pub struct StackifierTopologicalSort {
    pub last_popped_block: Option<BlockId>,
    top: BTreeMap<BlockId, StackifierDependency>,
    // complete topology will not change while popping
    pub complete_top: BTreeMap<BlockId, StackifierDependency>,
}

impl StackifierDependency {
    pub fn new() -> StackifierDependency {
        StackifierDependency {
            num_prec: 0,
            succ_set: BTreeSet::new(),
        }
    }
}

impl StackifierTopologicalSort {
    #[inline]
    pub fn new() -> StackifierTopologicalSort {
        StackifierTopologicalSort {
            last_popped_block: Option::None,
            top: BTreeMap::new(),
            complete_top: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.top.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.top.is_empty()
    }

    pub fn add_dependency(&mut self, prec: BlockId, succ: BlockId) {
        Self::add_dependency_impl(&mut self.top, prec, succ);
        Self::add_dependency_impl(&mut self.complete_top, prec, succ);
    }

    fn add_dependency_impl(
        top: &mut BTreeMap<BlockId, StackifierDependency>,
        prec: BlockId,
        succ: BlockId,
    ) {
        match top.entry(prec) {
            Entry::Vacant(e) => {
                let mut dep = StackifierDependency::new();
                dep.succ_set.insert(succ);
                e.insert(dep);
            }
            Entry::Occupied(e) => {
                e.into_mut().succ_set.insert(succ);
            }
        }
        match top.entry(succ) {
            Entry::Vacant(e) => {
                let mut dep = StackifierDependency::new();
                dep.num_prec = 1;
                e.insert(dep);
            }
            Entry::Occupied(e) => {
                e.into_mut().num_prec += 1;
            }
        }
    }

    pub fn pop(&mut self) -> Option<Vec<BlockId>> {
        if let Some(peek_blocks) = self.peek() {
            for peek_block in &peek_blocks {
                self.remove(*peek_block);
                self.last_popped_block = Some(*peek_block);
            }
            return Some(peek_blocks);
        }
        None
    }

    pub fn remove(&mut self, prec: BlockId) -> Option<StackifierDependency> {
        let result = self.top.remove(&prec);
        if let Some(ref p) = result {
            for s in &p.succ_set {
                if let Some(y) = self.top.get_mut(s) {
                    y.num_prec -= 1;
                }
            }
        }
        result
    }

    /// Instead of picking random block w/o precessor after popping the last block,
    /// we prioritize blocks with last popped block as their only precessor.
    /// Also if the prioritized block is also the only successor of last popped block,
    /// we will keep nesting them into one vector until either condition goes false.
    pub fn peek(&self) -> Option<Vec<BlockId>> {
        let mut priority_succ: Option<BlockId> = None;
        if let Some(last_block) = self.last_popped_block {
            priority_succ = self.find_priority_succ(last_block);
        }

        // prioritize priority_succ if any, otherwise
        // run original topological sorting.
        let mut nested_blocks: Vec<BlockId> = vec![];
        if let Some(priority_succ_blk_id) = priority_succ {
            nested_blocks.push(priority_succ_blk_id);
            let mut curr_block = priority_succ_blk_id;
            if let Some(curr_dependency) = self.complete_top.get(&curr_block) {
                let mut succ_set = &curr_dependency.succ_set;
                // nest blocks iff 1. curr_block has only one succ;
                // && 2. the succ has only one precessor
                while let Some(next_priority_succ_blk_id) = self.find_priority_succ(curr_block) {
                    if succ_set.len() != 1 {
                        break;
                    }
                    nested_blocks.push(next_priority_succ_blk_id);
                    curr_block = next_priority_succ_blk_id;
                    if let Some(dependency) = self.complete_top.get(&curr_block) {
                        succ_set = &dependency.succ_set;
                    }
                }
            }
        } else if let Some(next_succ) = self
            .top
            .iter()
            .filter(|&(_, v)| v.num_prec == 0)
            .map(|(k, _)| k)
            .next()
            .cloned()
        {
            nested_blocks.push(next_succ);
        } else {
            return None;
        }
        Some(nested_blocks)
    }

    //? Priority succ is a block whose only precessor is last block.
    pub fn find_priority_succ(&self, last_block: BlockId) -> Option<BlockId> {
        let mut priority_succ = None;
        if let Some(last_block_dependency) = self.complete_top.get(&last_block) {
            for succ in &last_block_dependency.succ_set {
                if let Some(complete_succ_dependency) = self.complete_top.get(succ) {
                    let succ_prec_num = complete_succ_dependency.num_prec;
                    if succ_prec_num == 1 {
                        priority_succ = Some(*succ);
                        break;
                    }
                }
            }
        }
        priority_succ
    }
}
