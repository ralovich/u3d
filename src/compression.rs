// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

//#![allow(unused, non_upper_case_globals)]
use bitstream_io::{BitRead, BitReader};
//use byteorder::ReadBytesExt;
use log::debug;
use std::collections::{BTreeMap, HashMap};
//use std::io::{Error, Read};

// use std::sync::atomic::{AtomicUsize, Ordering};
// static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
// macro_rules! PRINTU32 {
//     ($value:tt, $r:tt, $num_ctxs:tt) => {
//         let bp = $r.position_in_bits()?;
//         let c = CALL_COUNT.load(Ordering::SeqCst);
//         println!("U32 {} {} bp={} nc={}", $value, c, bp, $num_ctxs);
//         CALL_COUNT.fetch_add(1, Ordering::SeqCst);
//
//     };
// }

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum U3dContextKind {
    Context8 = 0,
    cPosDiffSign = 1,
    cPosDiffX,
    cPosDiffY,
    cPosDiffZ,
    cNormlCnt,
    cDiffNormalSign,
    cDiffNormalX,
    cDiffNormalY,
    cDiffNormalZ,
    cShading, // material
}
impl From<U3dContextKind> for u32 {
    fn from(val: U3dContextKind) -> Self {
        val as u32
    }
}

const STATIC_FULL: u32 = 0x00000400;
const MAX_RANGE: u32 = STATIC_FULL + 0x00003FFF;
const CONTEXT8: u32 = 0;

pub fn range(range: u32) -> u32 {
    STATIC_FULL + range
}

pub struct U3dDecoder {
    /**< initialy 0x0000FFFF        */
    high: u32,
    /**< initialy 0x00000000        */
    low: u32,
    /**< initially is 0             */
    under_flow_count: u32,
}
impl Default for U3dDecoder {
    fn default() -> Self {
        U3dDecoder {
            high: 0x0000FFFF,
            low: 0,
            under_flow_count: 0,
        }
    }
}

pub fn read_bits<R: BitRead>(r: &mut R, num_bits: u8) -> std::io::Result<u8> {
    let mut value: u8 = 0;
    for _i in 0..num_bits {
        //value <<= 1;
        let bit = r.read_bit()?;
        value |= ((bit as u8) & 0x01) << (_i);
    }
    Ok(value)
}
// pub fn write_bits<W: BitWrite + ?Sized>(w: &mut W, value: u8, num_bits: u8) -> std::io::Result<()> {
//     //assert!(num_bits == 1 || num_bits == 3 || num_bits == 4 || num_bits == 8);
//     for i in 0..num_bits {
//         let bit = 1 == (value >> (num_bits - 1 - i)) & 1;
//         w.write_bit(bit)?;
//     }
//     Ok(())
// }

pub fn read_u8<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<u8> {
    let value = read_bits(r, 8)?;
    //error!("R_U8:{} bp={}", value, r.position_in_bits()?);
    Ok(value)
}

pub fn read_15bits<R: BitRead>(r: &mut R) -> std::io::Result<u32> {
    let mut value: u32 = 0;
    for _i in 0..15 {
        let bit = r.read_bit()?;
        value = (value << 1) | (bit as u32);
    }
    Ok(value)
}

pub fn read_u16<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<u16> {
    let lo = read_u8(r)? as u16;
    let hi = read_u8(r)? as u16;
    Ok(lo | hi << 8)
}

pub fn read_i16<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<i16> {
    let i = read_u16(r)?;
    Ok(i as i16)
}

pub fn read_u32<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<u32> {
    Ok((read_u8(r)? as u32)
        | (read_u8(r)? as u32) << 8
        | (read_u8(r)? as u32) << 16
        | (read_u8(r)? as u32) << 24)
}

pub fn read_u64<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<u64> {
    let lo = read_u32(r)? as u64;
    let hi = read_u32(r)? as u64;
    Ok(lo | hi << 32)
}

pub fn read_f32<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<f32> {
    Ok(f32::from_bits(read_u32(r)?))
}

pub fn read_f64<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<f64> {
    Ok(f64::from_bits(read_u64(r)?))
}

// pub fn read_vec_f32(ref mut r: impl Read, x: u32, y: u32) -> Result<Vec<_>> {
//     let v: Vec<f32> = (0..x * y)
//         .map(|_| r.read_f32::<LittleEndian>())
//         .collect::<Result<_>>()?;
//     let v_reshaped = v.chunks(x as usize).collect();
//     Ok(v_reshaped)
// }

pub fn read_f32_3<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<[f32; 3]> {
    Ok([
        f32::from_bits(read_u32(r)?),
        f32::from_bits(read_u32(r)?),
        f32::from_bits(read_u32(r)?),
    ])
}

pub fn read_f32_4<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
    r: &mut BitReader<R, E>,
) -> std::io::Result<[f32; 4]> {
    Ok([
        f32::from_bits(read_u32(r)?),
        f32::from_bits(read_u32(r)?),
        f32::from_bits(read_u32(r)?),
        f32::from_bits(read_u32(r)?),
    ])
}

/// static histograms represent a uniform distribution over a range of numbers. The dynamic histograms build the
/// distribution from the values written using a context. The dynamic contexts are used for values
/// that are expected to have a narrow distribution.
/// the encoding algorithm is very order dependent.
#[derive(Default)]
pub struct U3dCompressionContexts {
    pub no_compression: bool,
    dec: U3dDecoder,
    mgr2: ContextManager,
}
impl U3dCompressionContexts {
    pub fn decoder_reset(&mut self) {
        self.dec = U3dDecoder::default();
    }
    pub fn release_decoder_compression_context(&mut self, _ctx: U3dContextKind) {
        self.mgr2.contexts.remove(&(_ctx as u32));
    }
    fn swap_bits8(value: u8) -> u8 {
        const SWAP8: [u8; 16] = [0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15];
        (SWAP8[(value & 0xf) as usize] << 4) | (SWAP8[(value >> 4) as usize])
    }
    fn read_u8<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        &mut self,
        r: &mut BitReader<R, E>,
    ) -> std::io::Result<u8> {
        let pos = r.position_in_bits()?;
        let control = read_u8(r)?;
        r.seek_bits(std::io::SeekFrom::Start(pos))?;

        if self.no_compression {
            return read_u8(r);
        }

        let mut value = self.read_symbol(r, CONTEXT8)?;
        value -= 1;
        let rv = Self::swap_bits8(value as u8);
        if control != rv {
            debug!("{} {}", control, rv);
        }
        Ok(rv)
    }
    fn read_u16<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        &mut self,
        r: &mut BitReader<R, E>,
    ) -> std::io::Result<u16> {
        let low = self.read_u8(r)?;
        let high = self.read_u8(r)?;
        Ok(low as u16 | ((high as u16) << 8))
    }
    fn read_u32<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        &mut self,
        r: &mut BitReader<R, E>,
    ) -> std::io::Result<u32> {
        let low = self.read_u16(r)?;
        let high = self.read_u16(r)?;
        Ok(low as u32 | ((high as u32) << 8))
    }
    /* ReadSymbol
     * Read a symbol from the datablock using the specified context.
     * The symbol 0 represents the escape value and signifies that the
     * next symbol read will be uncompressed.
     */
    pub fn read_symbol<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        &mut self,
        r: &mut BitReader<R, E>,
        ctx: u32,
    ) -> std::io::Result<u32> {
        // Fill in the code word
        // this is not a real 16 bit read, the cursor is not advanced...
        let pos = r.position_in_bits()?;
        let mut code: u32 = (read_bits(r, 1)? as u32) << 15;
        r.seek_bits(std::io::SeekFrom::Current(self.dec.under_flow_count as i64))?;
        // BUG FIXME: when on the last byte this "16" bit read fails, need to address that
        code |= read_15bits(r)?;
        r.seek_bits(std::io::SeekFrom::Start(pos))?;

        //assert!(self.mgr2.contexts.contains_key(&ctx));

        // Get total count to calculate probabilities
        let total_cum_freq: u32 = self.mgr2.get_total_symbol_frequency(ctx);
        if total_cum_freq < 1 {
            return Err(std::io::Error::other(format!(
                "Invalid histogram! total_cum_freq={}",
                total_cum_freq
            )));
        }
        // Get the cumulative frequency of the current symbol
        if self.dec.low > self.dec.high {
            return Err(std::io::Error::other("Invalid range!"));
        }
        let range = self.dec.high + 1 - self.dec.low;
        if range == 0 {
            return Err(std::io::Error::other("Empty range!"));
        }
        // The relationship:
        // code_cum_freq <= (total_cum_freq * (this.code - this.low)) / range
        // is used to calculate the cumulative frequency of the current
        // symbol. The +1 and -1 in the line below are used to counteract
        // finite word length problems resulting from the division by range.
        if self.dec.low > code {
            return Err(std::io::Error::other("Invalid state!"));
        }
        let code_cum_freq = (total_cum_freq * (1 + code - self.dec.low) - 1) / range;
        // Get the current symbol
        let value = self.mgr2.get_symbol_from_frequency(ctx, code_cum_freq);
        // Update state and context
        let value_cum_freq = self.mgr2.get_cumulative_symbol_frequency(ctx, value);
        let value_freq = self.mgr2.get_symbol_frequency(ctx, value);

        let mut low = self.dec.low;
        #[allow(unused_assignments)]
        let mut high = self.dec.high;

        // i64 dance is required for the case low==0 and thus (low-1) underflows...
        let tmp =
            low as i64 - 1i64 + (range * (value_cum_freq + value_freq) / total_cum_freq) as i64;
        high = tmp as u32;
        low += range * value_cum_freq / total_cum_freq;

        self.mgr2.add_symbol(ctx, value);

        // Fast count the first 4 bits
        const READ_COUNT: [u32; 16] = [4, 3, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut bit_count: u32 = READ_COUNT[(((low >> 12) ^ (high >> 12)) & 0x0000000F) as usize];
        const FAST_NOT_MASK: [u32; 5] =
            [0x0000FFFF, 0x00007FFF, 0x00003FFF, 0x00001FFF, 0x00000FFF];
        low &= FAST_NOT_MASK[bit_count as usize];
        high &= FAST_NOT_MASK[bit_count as usize];
        high <<= bit_count;
        low <<= bit_count;
        high |= (1u32 << bit_count) - 1;

        // Regular count the rest
        const HALF_MASK: u32 = 0x00008000;
        const NOT_HALF_MASK: u32 = 0x00007FFF;
        const QUARTER_MASK: u32 = 0x00004000;
        const NOT_THREE_QUARTER_MASK: u32 = 0x00003FFF;
        let mut masked_low = HALF_MASK & low;
        let mut masked_high = HALF_MASK & high;

        while ((masked_low | masked_high) == 0)
            || ((masked_low == HALF_MASK) && masked_high == HALF_MASK)
        {
            low = (NOT_HALF_MASK & low) << 1;
            high = ((NOT_HALF_MASK & high) << 1) | 1;
            masked_low = HALF_MASK & low;
            masked_high = HALF_MASK & high;
            bit_count += 1;
        }
        let saved_bits_low = masked_low;
        let saved_bits_high = masked_high;
        if bit_count > 0 {
            bit_count += self.dec.under_flow_count;
            self.dec.under_flow_count = 0;
        }

        // Count underflow bits
        masked_low = QUARTER_MASK & low;
        masked_high = QUARTER_MASK & high;

        let mut underflow = 0;
        while masked_low == 0x4000 && masked_high == 0 {
            low &= NOT_THREE_QUARTER_MASK;
            high &= NOT_THREE_QUARTER_MASK;
            low += low;
            high += high;
            high |= 1;
            masked_low = QUARTER_MASK & low;
            masked_high = QUARTER_MASK & high;
            underflow += 1;
        }

        // Store the state
        self.dec.under_flow_count += underflow;
        low |= saved_bits_low;
        high |= saved_bits_high;
        self.dec.low = low;
        self.dec.high = high;

        // Update bit read position
        r.seek_bits(std::io::SeekFrom::Current(bit_count as u8 as i64))?;

        Ok(value)
    }
    pub fn read_compressed_u32<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        &mut self,
        r: &mut BitReader<R, E>,
        ctx: u32,
    ) -> std::io::Result<u32> {
        //let _c = CALL_COUNT.load(Ordering::SeqCst);
        if !self.no_compression && ContextManager::is_context_compressed(ctx) {
            let symbol = self.read_symbol(r, ctx)?;
            if symbol != 0 {
                //the symbol is compressed
                let value = symbol - 1;
                //let num_ctxs = self.mgr2.contexts.len();
                //PRINTU32!(value, r, num_ctxs);
                return Ok(value);
            } else {
                //escape character, the symbol was not compressed
                let value = self.read_u32(r)?;
                self.mgr2.add_symbol(ctx, value + 1);
                //let num_ctxs = self.mgr2.contexts.len();
                //PRINTU32!(value, r, num_ctxs);
                return Ok(value);
            }
        }

        let value = self.read_u32(r)?;
        //let num_ctxs = self.mgr2.contexts.len();
        //PRINTU32!(value, r, num_ctxs);
        Ok(value)
    }
}

#[derive(Default)]
struct DynamicHistogramData {
    /// number of occurrences of each symbol
    symbol_count: u32,
    /// the value is the number of occurrences of a symbol and every symbol with a larger value
    cumulative_symbol_count: u32,
}

struct DynamicHistogram {
    /// key is symbol, BTree so it's sorted
    symbols: BTreeMap<u32, DynamicHistogramData>,
}
impl Default for DynamicHistogram {
    fn default() -> Self {
        let mut symbols = BTreeMap::new();
        symbols.insert(
            0,
            DynamicHistogramData {
                symbol_count: 1,
                cumulative_symbol_count: 1,
            },
        );
        Self { symbols }
    }
}
#[derive(Default)]
struct ContextManager {
    /// key is context id
    contexts: HashMap<u32, DynamicHistogram>,
}
impl ContextManager {
    pub fn get_total_symbol_frequency(&self, ctx: u32) -> u32 {
        if Self::is_context_dynamic_and_compressed(ctx) {
            if self.contexts.contains_key(&ctx) {
                self.contexts[&ctx].symbols[&0].cumulative_symbol_count
            } else {
                1
            }
        } else if ctx == CONTEXT8 {
            256
        } else {
            ctx - STATIC_FULL
        }
    }
    pub fn get_symbol_frequency(&self, ctx: u32, symbol: u32) -> u32 {
        if Self::is_context_dynamic_and_compressed(ctx) {
            if self.contexts.contains_key(&ctx) {
                if self.contexts[&ctx].symbols.contains_key(&symbol) {
                    return self.contexts[&ctx].symbols[&symbol].symbol_count;
                }
            } else if symbol == 0 {
                //if the histogram hasn't been created yet, the symbol 0 is
                //the escape value and should return 1
                return 1;
            }
            //the default for the dynamic case is 0
            return 0;
        }
        //the static case is 1.
        1
    }
    pub fn get_cumulative_symbol_frequency(&self, ctx: u32, symbol: u32) -> u32 {
        if Self::is_context_dynamic_and_compressed(ctx) {
            if self.contexts.contains_key(&ctx) {
                if self.contexts[&ctx].symbols.contains_key(&symbol) {
                    return self.contexts[&ctx].symbols[&0].cumulative_symbol_count
                        - self.contexts[&ctx].symbols[&symbol].cumulative_symbol_count;
                }
                return self.contexts[&ctx].symbols[&0].cumulative_symbol_count;
            }
            return 0;
        }
        symbol - 1
    }
    pub fn get_symbol_from_frequency(&self, ctx: u32, symbol_frequency: u32) -> u32 {
        if Self::is_context_dynamic_and_compressed(ctx) {
            if self.contexts.contains_key(&ctx)
                && symbol_frequency > 0
                && self.contexts[&ctx].symbols[&0].cumulative_symbol_count >= symbol_frequency
            {
                let mut value = 0;
                for i in &self.contexts[&ctx].symbols {
                    if self.get_cumulative_symbol_frequency(ctx, *i.0) <= symbol_frequency {
                        value = *i.0;
                    } else {
                        break;
                    }
                }
                return value;
            }
            return 0;
        }
        symbol_frequency + 1
    }
    pub fn add_symbol(&mut self, ctx: u32, symbol: u32) {
        /// ELEPHANT = Max cumulative symbol count
        const ELEPHANT: u32 = 0x00001fff;
        const MAXIMUM_SYMBOL_IN_HISTOGRAM: u32 = 0x0000FFFF;

        //check if dynamic. nothing to do if static or if the
        //symbol is larger than the maximum symbol allowed in the
        //histogram
        if !(Self::is_context_dynamic_and_compressed(ctx) && symbol < MAXIMUM_SYMBOL_IN_HISTOGRAM) {
            return;
        }

        self.contexts.entry(ctx).or_default();

        if self.contexts[&ctx].symbols[&0].cumulative_symbol_count >= ELEPHANT {
            //if total number of occurrences is larger than ELEPHANT,
            //scale down the values to avoid overflow
            let mut temp_accum = 0;
            for it in self
                .contexts
                .get_mut(&ctx)
                .unwrap()
                .symbols
                .iter_mut()
                .rev()
            {
                it.1.symbol_count >>= 1;
                temp_accum += it.1.symbol_count;
                it.1.cumulative_symbol_count = temp_accum;
            }

            //preserve the initial escape value of 1 for the symbol
            //count and cumulative count
            self.contexts
                .get_mut(&ctx)
                .unwrap()
                .symbols
                .get_mut(&0)
                .unwrap()
                .symbol_count += 1;
            self.contexts
                .get_mut(&ctx)
                .unwrap()
                .symbols
                .get_mut(&0)
                .unwrap()
                .cumulative_symbol_count += 1;
        }

        if !self.contexts[&ctx].symbols.contains_key(&symbol) {
            self.contexts
                .get_mut(&ctx)
                .unwrap()
                .symbols
                .insert(symbol, Default::default());
            //self.contexts.get_mut(&ctx).unwrap().symbols.get_mut(&symbol).unwrap().cumulative_symbol_count += 1;

            let symbols = &mut self.contexts.get_mut(&ctx).unwrap().symbols;
            let mut it = symbols.iter_mut();
            let mut current_elem;
            loop {
                current_elem = it.next();
                if *current_elem.as_ref().unwrap().0 == symbol {
                    let next_elem = it.next();
                    if let Some(next) = next_elem {
                        current_elem.unwrap().1.cumulative_symbol_count =
                            next.1.cumulative_symbol_count;
                    }
                    break;
                }
            }
        }

        self.contexts
            .get_mut(&ctx)
            .unwrap()
            .symbols
            .get_mut(&symbol)
            .unwrap()
            .symbol_count += 1;
        for it in self.contexts.get_mut(&ctx).unwrap().symbols.iter_mut() {
            if *it.0 > symbol {
                break;
            }
            it.1.cumulative_symbol_count += 1;
        }
    }
    pub fn is_context_compressed(ctx: u32) -> bool {
        ctx < MAX_RANGE && ctx != CONTEXT8
    }
    fn is_context_dynamic_and_compressed(ctx: u32) -> bool {
        ctx < STATIC_FULL && ctx != CONTEXT8
    }
    #[allow(unused)]
    fn dump(&self, ctx: u32) {
        if !self.contexts.contains_key(&ctx) {
            return;
        }
        let symbols = &self.contexts[&ctx].symbols;
        for i in symbols.iter() {
            println!(
                "s={}: sc={} csc={}",
                *i.0, i.1.symbol_count, i.1.cumulative_symbol_count
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap8() {
        let a = 0xFFu8;
        assert_eq!(U3dCompressionContexts::swap_bits8(a), 0xFF);

        let a = 0xF0u8;
        assert_eq!(U3dCompressionContexts::swap_bits8(a), 0x0F);

        let a = 0x80u8;
        assert_eq!(U3dCompressionContexts::swap_bits8(a), 0x01);

        let a = 0x40u8;
        assert_eq!(U3dCompressionContexts::swap_bits8(a), 0x02);
    }

    #[test]
    fn test_dyn_hist() {
        let mut c: ContextManager = Default::default();
        assert_eq!(c.contexts.len(), 0);

        let ctx = 1u32;
        c.add_symbol(ctx, 33);
        c.add_symbol(ctx, 33);
        c.add_symbol(ctx, 33);
        //c.dump(ctx);
        c.add_symbol(ctx, 6);
        //c.dump(ctx);
        c.add_symbol(ctx, 5);
        c.add_symbol(ctx, 34);
        c.add_symbol(ctx, 5);
        //c.dump(ctx);
        assert_eq!(c.get_symbol_frequency(ctx, 33), 3);
        assert_eq!(c.get_cumulative_symbol_frequency(ctx, 33), 4);
        assert_eq!(c.get_total_symbol_frequency(ctx), 8);
        assert_eq!(c.get_symbol_from_frequency(ctx, 4), 33);
        assert_eq!(c.get_symbol_frequency(ctx, 5), 2);
        assert_eq!(c.get_cumulative_symbol_frequency(ctx, 5), 1);
        assert_eq!(c.get_symbol_from_frequency(ctx, 2), 5);
        assert_eq!(c.get_symbol_frequency(ctx, 50), 0);
        assert_eq!(c.get_cumulative_symbol_frequency(ctx, 50), 8);
        c.dump(ctx);

        c.add_symbol(CONTEXT8, 5);
        assert_eq!(c.get_symbol_frequency(CONTEXT8, 5), 1);
        assert_eq!(c.get_cumulative_symbol_frequency(CONTEXT8, 5), 4);
        assert_eq!(c.get_total_symbol_frequency(CONTEXT8), 256);
        assert_eq!(c.get_symbol_frequency(CONTEXT8, 50), 1);
        assert_eq!(c.get_cumulative_symbol_frequency(CONTEXT8, 50), 49);

        let num_points = 533;
        let ctx = STATIC_FULL + num_points;
        assert!(ctx < MAX_RANGE);
        c.add_symbol(ctx, 5);
        assert_eq!(c.get_total_symbol_frequency(ctx), num_points);
    }
}
