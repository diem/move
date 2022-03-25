use anyhow::{Error, Result};
use move_binary_format::deserializer::{load_constant_size, load_signature_token};
use move_binary_format::file_format::SignatureToken;
use move_binary_format::file_format_common::{
    write_u64_as_uleb128, BinaryData, TableType, VersionedCursor,
};

use crate::context::TableContext;
use crate::mutator::Mutator;

#[derive(Debug)]
pub struct AddressAdaptation {
    source_len: usize,
    target_len: usize,
}

impl AddressAdaptation {
    pub fn new(source_len: usize, target_len: usize) -> AddressAdaptation {
        AddressAdaptation {
            source_len,
            target_len,
        }
    }

    #[inline]
    fn is_source_bigger(&self) -> bool {
        self.source_len > self.target_len
    }

    #[inline]
    fn len_diff(&self) -> usize {
        if self.is_source_bigger() {
            self.source_len - self.target_len
        } else {
            self.target_len - self.source_len
        }
    }

    #[inline]
    fn ilen_diff(&self) -> isize {
        self.target_len as isize - self.source_len as isize
    }

    pub fn make(&self, bytes: &mut Vec<u8>) -> Result<()> {
        if self.source_len == self.target_len {
            return Ok(());
        }

        let mut cursor = VersionedCursor::new(bytes.as_slice())
            .map_err(|err| Error::msg(format!("{:?}", err)))?;

        let mut mutator = Mutator::new();
        self.calc_diff(&mut cursor, &mut mutator)?;
        mutator.mutate(bytes);
        Ok(())
    }

    fn calc_diff(&self, cur: &mut VersionedCursor, mutator: &mut Mutator) -> Result<()> {
        let table_len = cur.read_uleb128_as_u64()?;

        let header_len = cur.position() as u32;
        let header_size = self.calc_header_size(cur, table_len)?;

        let mut additional_offset: i32 = 0;
        for _ in 0..table_len {
            let kind = cur.read_u8()?;

            let offset = if additional_offset != 0 {
                let start_pos = cur.position();
                let offset = cur.read_uleb128_as_u64()? as u32;
                self.make_uleb128_diff(
                    start_pos,
                    cur.position(),
                    (offset as i32 + additional_offset) as u32,
                    mutator,
                )?;
                offset
            } else {
                cur.read_uleb128_as_u64()? as u32
            };

            let t_len_start_pos = cur.position();
            let t_len = cur.read_uleb128_as_u64()? as u32;
            let t_len_end_pos = cur.position();

            let offset_diff = if kind == TableType::ADDRESS_IDENTIFIERS as u8 {
                self.handle_address_identifiers(
                    TableContext::new(cur, offset + header_size + header_len, t_len),
                    mutator,
                )
            } else if kind == TableType::CONSTANT_POOL as u8 {
                self.handle_const_pool(
                    TableContext::new(cur, offset + header_size + header_len, t_len),
                    mutator,
                )
                .map_err(|err| anyhow!("{:?}", err))?
            } else {
                0
            };

            if offset_diff != 0 {
                self.make_uleb128_diff(
                    t_len_start_pos,
                    t_len_end_pos,
                    (t_len as i32 + offset_diff) as u32,
                    mutator,
                )?;
            }

            additional_offset += offset_diff;
        }

        Ok(())
    }

    fn calc_header_size(&self, cur: &mut VersionedCursor, table_len: u64) -> Result<u32> {
        let start = cur.position() as u32;

        for _ in 0..table_len {
            cur.read_u8()?;
            cur.read_uleb128_as_u64()?;
            cur.read_uleb128_as_u64()?;
        }

        let end = cur.position() as u32;
        cur.set_position(start as u64);
        Ok(end - start)
    }

    fn make_uleb128_diff(
        &self,
        start_pos: u64,
        end_pos: u64,
        new_offset: u32,
        mutator: &mut Mutator,
    ) -> Result<()> {
        let mut binary = BinaryData::new();
        write_u64_as_uleb128(&mut binary, new_offset as u64)?;
        mutator.add_patch(start_pos as usize, end_pos as usize, binary.into_inner());
        Ok(())
    }

    fn handle_address_identifiers(&self, ctx: TableContext, mutator: &mut Mutator) -> i32 {
        if ctx.len != 0 {
            let mut offset_diff = 0;
            let diff = self.len_diff();
            for idx in (0..ctx.len).step_by(self.source_len) {
                let index = ctx.position() + idx as usize;
                if self.is_source_bigger() {
                    offset_diff -= diff as i32;
                    mutator.add_patch(index, index + diff, vec![]);
                } else {
                    offset_diff += diff as i32;
                    mutator.add_patch(index, index, vec![0x0; diff]);
                }
            }
            offset_diff
        } else {
            0
        }
    }

    fn handle_const_pool(&self, ctx: TableContext, mutator: &mut Mutator) -> Result<i32> {
        let end_offset = ctx.cursor.position() + ctx.len as u64;
        let mut additional_offset: i32 = 0;

        let diff_size = self.ilen_diff();
        while ctx.cursor.position() < end_offset {
            let type_ = load_signature_token(ctx.cursor).map_err(|err| anyhow!("{:?}", err))?;

            let size_start_offset = ctx.cursor.position();
            let size = load_constant_size(ctx.cursor).map_err(|err| anyhow!("{:?}", err))? as u32;
            let size_end_offset = ctx.cursor.position();

            if SignatureToken::Address == type_ {
                self.make_uleb128_diff(
                    size_start_offset,
                    size_end_offset,
                    (size as i32 + diff_size as i32) as u32,
                    mutator,
                )?;
                additional_offset += diff_size as i32;
                let index = ctx.cursor.position() as usize;
                if self.is_source_bigger() {
                    mutator.add_patch(index, index + diff_size.abs() as usize, vec![]);
                } else {
                    mutator.add_patch(index, index, vec![0x0; diff_size.abs() as usize]);
                }
            }
            ctx.cursor.set_position(ctx.cursor.position() + size as u64);
        }

        Ok(additional_offset)
    }
}
