// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

use crate::blocks::{
    CLODBaseMeshContinuation, CLODMeshDeclaration, CLODProgressiveMeshContinuation, MetaData,
    UString,
};
use crate::compression::*;
use bitstream_io::BitReader;
use byteorder::{LittleEndian, ReadBytesExt};
use log::{debug, info, trace, warn};
use measure_time::debug_time;
use num_enum::TryFromPrimitive;
use std::fmt;
use std::io::{Cursor, SeekFrom};
use std::io::{Read, Seek};

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, TryFromPrimitive)]
#[repr(u32)]
pub enum U3dBlockKind {
    FileHeader = 0x00443355,

    FileReference = 0xFFFFFF12,
    ModifierChain = 0xFFFFFF14,
    PriorityUpdate = 0xFFFFFF15,
    NewObjectType = 0xFFFFFF16,

    GroupNode = 0xFFFFFF21,
    NodeModel = 0xFFFFFF22,
    LightNode = 0xFFFFFF23,
    NodeView = 0xFFFFFF24,

    CLODMeshDeclaration = 0xFFFFFF31,
    CLODBaseMeshContinuation = 0xFFFFFF3B,
    CLODProgressiveMeshContinuation = 0xFFFFFF3C,
    LineSetContinuation = 0xFFFFFF3F,
    ModifierShading = 0xFFFFFF45,
    ModifierCLOD = 0xFFFFFF46,

    LightResource = 0xFFFFFF51,
    ViewResource = 0xFFFFFF52,
    LitTextureShader = 0xFFFFFF53,
    MaterialResource = 0xFFFFFF54,
    TextureDeclaration = 0xFFFFFF55,
    MotionResource = 0xFFFFFF56,
    TextureContinuation = 0xFFFFFF5C,
}
impl fmt::Display for U3dBlockKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} (0x{:08x})", self, *self as u32)
    }
}

pub struct Block {
    pub block_type: u32,
    pub data: Vec<u8>,
    pub meta_data: Vec<u8>,
}
impl Block {
    pub fn from_reader(mut r: impl Read + Seek) -> std::io::Result<Self> {
        //let start = r.stream_position()?;
        let block_type = r.read_u32::<LittleEndian>()?;
        //debug!("block_type: 0x{:08x}", block_type);
        let data_size = r.read_u32::<LittleEndian>()?;
        let meta_data_size = r.read_u32::<LittleEndian>()?;
        let data: Vec<u8> = (0..data_size)
            .map(|_| r.read_u8())
            .collect::<std::io::Result<_>>()?;
        let num_data_padding_bytes = (4 - (data_size % 4)) % 4;
        assert!(
            /*num_data_padding_bytes >= 0 &&*/ num_data_padding_bytes <= 3
        );
        r.seek(SeekFrom::Current(num_data_padding_bytes as i64))?;
        //let _padding: Vec<u8> = (0..num_data_padding_bytes)
        //    .map(|_| r.read_u8())
        //    .collect::<Result<_>>()?;
        let meta_data: Vec<u8> = (0..meta_data_size)
            .map(|_| r.read_u8())
            .collect::<std::io::Result<_>>()?;
        let num_meta_data_padding_bytes = (4 - (meta_data.len() % 4)) % 4;
        r.seek(SeekFrom::Current(num_meta_data_padding_bytes as i64))?;
        //let meta_data = MetaData::from_reader(&mut *r)?;
        let rv = Self {
            block_type,
            data,
            meta_data,
        };
        Ok(rv)
    }
}

#[derive(Default)]
pub struct ParsedU3d {
    /// blocks parsed successfully
    pub num_blocks: u32,
    /// failed to parse
    pub num_failed: u32,
    /// unknown block kind, unimplemented
    pub num_unknown: u32,
}

#[derive(Default)]
pub struct ParsingContext {
    pub file_name: String,
    pub contexts: U3dCompressionContexts,

    pub meshes: Vec<CLODMeshDeclaration>,
    pub meshes2: Vec<CLODBaseMeshContinuation>,
}
impl ParsingContext {
    fn set_mesh_decl(&mut self, mesh_decl: CLODMeshDeclaration) {
        self.meshes.push(mesh_decl);
    }
    fn set_mesh_cont(&mut self, mesh: CLODBaseMeshContinuation) {
        self.meshes2.push(mesh);
        //assert_eq!(self.m)
    }

    pub fn find_mesh_decl(&self, name: &String) -> Option<CLODMeshDeclaration> {
        for m in &self.meshes {
            if m.name.value == *name {
                return Some(m.clone());
            }
        }
        None
    }
}

pub fn u3d_parse(bytes: &[u8], file_name: &String) -> std::io::Result<ParsedU3d> {
    if bytes.len() < 4
        || bytes[0] != b'U'
        || bytes[1] != b'3'
        || bytes[2] != b'D'
        || bytes[3] != b'\0'
    {
        return Err(std::io::Error::other("Wrong U3D signature!"));
    }
    debug!("U3D signature OK");

    //let mut contexts: U3dCompressionContexts = Default::default();
    let mut mem_reader = Cursor::new(bytes);
    let mut ctx = ParsingContext {
        file_name: file_name.to_string(),
        ..Default::default()
    };
    let mut parsed: ParsedU3d = Default::default();
    let mut header_present = false;
    loop {
        let p = mem_reader.position();
        let r = Block::from_reader(&mut mem_reader);
        let b: Block = match r {
            Ok(block) => block,
            Err(_why) => break,
        };

        if !header_present && b.block_type == U3dBlockKind::FileHeader as u32 {
            header_present = true;
        }

        if b.block_type == 0 {
            continue;
        }

        let bt = U3dBlockKind::try_from(b.block_type);
        match bt {
            Ok(bt) => {
                let _parsed = parse_block(bt, p, &b, &mut ctx, false);
                if _parsed.is_ok() {
                    parsed.num_blocks += 1;
                } else {
                    parsed.num_failed += 1;
                    warn!("Failed to parse {}: {}", bt, _parsed.err().unwrap());
                }
            }
            Err(_why) => {
                parsed.num_unknown += 1;
                trace!(
                    "skipping unimplemented 0x{:08x} U3D block type",
                    b.block_type
                );
            }
        }
    }
    if !header_present {
        warn!("No {} present", U3dBlockKind::FileHeader);
    }

    info!(
        "U3D parsed {} blocks ( {} failed, {} unimplemented)",
        parsed.num_blocks, parsed.num_failed, parsed.num_unknown
    );
    info!(
        "--parsed successfully \"{}\": {} bytes, {} blocks--",
        file_name,
        mem_reader.position(),
        parsed.num_blocks
    );
    if mem_reader.position() != bytes.len() as u64 {
        warn!(
            "{} uninterpreted trailing bytes",
            bytes.len() as i64 - mem_reader.position() as i64
        );
    }
    Ok(parsed)
}

pub fn u3d_parse_file(file_name: &String) -> std::io::Result<ParsedU3d> {
    debug_time!("u3d_parse_file");
    //let path = Path::new(file_name);
    //let display = path.display();

    info!("--parsing \"{}\"--", file_name);

    let bytes: Vec<u8> = std::fs::read(file_name)?;
    //let len = bytes.len();
    debug!("read {} bytes", bytes.len());

    u3d_parse(bytes.as_slice(), file_name)
}

pub fn parse_block(
    k: U3dBlockKind,
    start_pos: u64,
    b: &Block,
    ctx: &mut ParsingContext,
    skip_metadata: bool,
) -> std::io::Result<()> {
    trace!("parse_block() {} byp={}", k, start_pos);
    let endian = bitstream_io::LittleEndian;
    match k {
        U3dBlockKind::FileHeader => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let obj = crate::blocks::FileHeader::from_reader(&mut r, &mut ctx.contexts)?;
            trace!("{:#?}", obj);
        }
        U3dBlockKind::FileReference => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let obj = crate::blocks::FileReference::from_reader(&mut r, &mut ctx.contexts)?;
            trace!("{:#?}", obj);
            //Ok(())
        }
        U3dBlockKind::ModifierChain => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let _obj = crate::blocks::ModifierChain::from_reader(&mut r, ctx)?;
            //trace!("{:#?}", _obj);
            if r.position_in_bits()? != (b.data.len() as u64) * 8 {
                warn!(
                    "read: {}, len was {} bits",
                    r.position_in_bits()?,
                    (b.data.len() as u64) * 8
                );
            }
            //Ok(())
        }
        U3dBlockKind::PriorityUpdate => {
            let mut r = &b.data[..];
            let new_priority = r.read_u32::<LittleEndian>()?;
            trace!("\tnew_priority: {}", new_priority);
            //Ok(())
        }
        U3dBlockKind::NewObjectType => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let name = UString::from_reader(&mut r)?;
            let _mod_type = read_u32(&mut r)?;
            let ext_id = crate::blocks::Uuid::from_reader(&mut r)?;
            let _new_block_type = read_u32(&mut r)?;
            let _cont_block_count = read_u32(&mut r)?;
            let _new_cont_block_type: Vec<u32> = (0.._cont_block_count)
                .map(|_| read_u32(&mut r))
                .collect::<std::io::Result<_>>()?;
            let ext_vendor_name = UString::from_reader(&mut r)?;
            if name.value == "RHAdobeMeshResource"
                || ext_id
                    == (crate::blocks::Uuid {
                        a: 0x96a804a6,
                        b: 0x3fb9,
                        c: 0x43c5,
                        d: [0xb2, 0xdf, 0x2a, 0x31, 0xb5, 0x56, 0x93, 0x40],
                    })
                || ext_vendor_name.value == "Right Hemisphere Adobe Systems"
            {
                debug!("RHAdobeMeshResource {} {}", name, ext_vendor_name);
            }
            let ext_url_count = read_u32(&mut r)?;
            let _ext_info_urls: Vec<UString> = (0..ext_url_count)
                .map(|_| UString::from_reader(&mut r))
                .collect::<std::io::Result<_>>()?;
            let ext_info_string = UString::from_reader(&mut r)?;
            debug!("{}", ext_info_string);
            if ext_info_string.value != "version 1.0" {
                return Err(std::io::Error::other(
                    "Unexpected RHAdobeMeshResource version!",
                ));
            }
            //Ok(())
        }
        U3dBlockKind::GroupNode => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let name = UString::from_reader(&mut r)?;
            let parent_node_count = read_u32(&mut r)?;
            let parent_node_name = UString::from_reader(&mut r)?;
            debug!(
                "{}: {} parent_node_count={} {}",
                k, name, parent_node_count, parent_node_name
            );
            let mut matrix: [f32; 16] = Default::default();
            for i in 0..16 {
                matrix[i as usize] = f32::from_bits(read_u32(&mut r)?);
            }
            //Ok(())
        }
        U3dBlockKind::NodeModel | U3dBlockKind::LightNode | U3dBlockKind::NodeView => {}
        U3dBlockKind::CLODMeshDeclaration => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let obj = CLODMeshDeclaration::from_reader(&mut r, &mut ctx.contexts)?;
            trace!("{:#?}", obj);
            ctx.set_mesh_decl(obj);
            //Ok(())
        }
        U3dBlockKind::CLODBaseMeshContinuation => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let obj = CLODBaseMeshContinuation::from_reader(&mut r, ctx)?;
            //trace!("{:#?}", obj);

            let base = std::path::Path::new(&ctx.file_name);
            let base_name = base.file_stem().unwrap().to_str().unwrap();
            let _file_name = format!("{}_{}.obj", base_name, start_pos);
            //obj.save(&_file_name)?;

            ctx.set_mesh_cont(obj);
            //Ok(())
        }
        U3dBlockKind::CLODProgressiveMeshContinuation => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let obj = CLODProgressiveMeshContinuation::from_reader(&mut r, ctx)?;
            trace!("{:#?}", obj);
        }
        U3dBlockKind::LineSetContinuation => {}
        U3dBlockKind::ModifierShading | U3dBlockKind::ModifierCLOD => {
            //Ok(())
        }
        U3dBlockKind::LightResource | U3dBlockKind::ViewResource => {
            //Ok(())
        }
        U3dBlockKind::LitTextureShader => {
            //Ok(())
        }
        U3dBlockKind::MaterialResource => {
            let mut r = BitReader::endian(Cursor::new(b.data.as_slice()), endian);
            let _obj = crate::blocks::MaterialResource::from_reader(&mut r, &mut ctx.contexts)?;
            //Ok(())
        }
        U3dBlockKind::TextureDeclaration
        | U3dBlockKind::MotionResource
        | U3dBlockKind::TextureContinuation => {
            //Ok(())
        }
    };
    if !skip_metadata && !b.meta_data.is_empty() {
        let mut r = BitReader::endian(Cursor::new(b.meta_data.as_slice()), endian);
        let obj = MetaData::from_reader(&mut r)?;
        trace!("{:#?}", obj);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_u3d() {
        let path = std::env::current_dir().unwrap();
        println!(
            "[test_load_u3d] The current directory is {}",
            path.display()
        );

        let rv = u3d_parse_file(&"testdata/pdf3d.stream-11.u3d".to_owned());
        assert!(rv.is_ok());

        let rv = rv.unwrap();
        println!("[test_load_u3d] {}", rv.num_blocks);
        assert_eq!(rv.num_blocks, 6);
    }

    #[test]
    fn test_load_u3d_fuzz() {
        let file_name = "fuzz.u3d".to_owned();

        let bytes = [
            85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 255, 255, 255, 19, 0, 0, 0, 0, 0, 0, 0, 4,
            0, 0, 0, 0, 0, 0, 0, 0, 255, 247, 0, 0, 0, 255, 112, 0, 0, 0, 0, 49, 255, 255, 255,
            143, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 153, 1, 0, 0, 0, 0, 0, 0, 78, 0, 2, 0, 0, 0,
            1, 0, 0, 20, 0, 0, 246, 255, 255, 255, 255, 0, 0, 255, 209, 1, 0, 0, 0, 0, 0, 0, 0, 2,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 134, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 246, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 246, 255, 255, 255, 255, 0, 0, 255, 209, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 115, 0, 0, 0, 0, 85, 54, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255,
            255, 184, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 254, 255, 255, 59, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 1, 9, 0, 7, 0, 255, 112, 0, 0,
            0, 0, 49, 255, 255, 255, 143, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 153, 1, 0, 0, 0, 0,
            0, 0, 78, 0, 2, 0, 0, 0, 1, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 255, 255, 255, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255,
            126, 255, 255, 255, 255, 255, 255, 0, 255, 43, 255, 255, 255, 148, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 255, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 213, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 22, 255, 255, 255,
            13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 22, 255, 255,
            255, 13, 0, 0, 0, 0, 0, 0, 201, 0, 0, 5, 0, 0, 213, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 41, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 20, 0, 0, 246, 255, 255, 255, 255, 0, 0, 255, 209, 1,
            0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 68, 0, 0, 0, 43, 0, 0, 0,
            0,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());
        assert_eq!(rv.as_ref().unwrap().num_blocks, 1);
        assert_eq!(rv.as_ref().unwrap().num_failed, 2);
        assert_eq!(rv.as_ref().unwrap().num_unknown, 2);

        let bytes = [
            85, 51, 68, 0, 48, 0, 0, 0, 8, 0, 0, 0, 72, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 65, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 85, 51, 68, 0, 0, 0,
            1, 255, 0, 0, 178, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 1, 17,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());
        assert_eq!(rv.as_ref().unwrap().num_blocks, 0);
        assert_eq!(rv.as_ref().unwrap().num_failed, 1);
        assert_eq!(rv.as_ref().unwrap().num_unknown, 0);

        let bytes = [
            85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 18, 255, 255, 255, 12, 0, 0, 0, 5, 0, 0, 0, 0,
            0, 0, 0, 12, 0, 255, 255, 255, 255, 255, 255, 255, 80, 255, 0, 0,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());
        assert_eq!(rv.as_ref().unwrap().num_blocks, 0);
        assert_eq!(rv.as_ref().unwrap().num_failed, 2);
        assert_eq!(rv.as_ref().unwrap().num_unknown, 0);
    }

    #[allow(unused)]
    #[test]
    #[ignore = "slow"]
    fn test_fuzz2() {
        let file_name = "fuzz.u3d".to_owned();

        // signal
        let bytes = [
            85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 49, 255, 255, 255, 149, 0, 0, 0, 249, 0, 0, 0,
            0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 85, 51,
            68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 255, 255, 255, 5, 0, 0, 0, 249, 0, 0, 0, 0, 0, 0, 0,
            2, 0, 0, 0, 0, 20, 255, 255, 255, 45, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 20, 255, 255, 255,
            45, 0, 0, 0, 0, 139, 155, 155, 155, 155, 155, 155, 101, 155, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 255, 255, 255, 45, 0, 0, 0, 11, 0, 0, 85, 51, 68, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 20, 255, 255, 255, 45, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 20, 255,
            255, 255, 45, 0, 0, 0, 0, 139, 155, 155, 155, 155, 155, 155, 101, 155, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 255, 255, 255, 45, 0, 0, 0, 11, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 85, 51, 68, 0, 0, 0, 0, 2, 255, 255, 45,
            0, 0, 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 70, 255,
            255, 255, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 200, 0, 0, 0, 214, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255,
            0, 255, 68, 77, 0, 0, 0, 0, 1, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 103, 1, 59, 1, 0, 0, 0, 0, 0, 2, 20, 116, 116, 116, 85, 49, 68, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 55, 0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 55, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 255, 255, 255,
            7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 100, 100, 100, 0, 255, 255, 255, 51, 68, 0, 39, 0,
            1, 0, 0, 0, 99, 99, 99, 99, 99, 99, 99, 0, 0, 0, 35, 0, 0, 0, 99, 0,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());

        // signal
        let bytes = [
            85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 255, 255, 255, 19, 0, 0, 0, 0, 0, 0, 0, 4,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 246, 0, 0, 255, 255, 0, 0, 0, 0, 49, 255, 255, 255,
            158, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 85, 51, 68, 0, 59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 255, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 0, 0, 0, 0, 18, 255, 255,
            255, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 226, 226, 226, 226, 226, 226, 226, 226,
            226, 226, 226, 226, 226, 226, 226, 226, 226, 226, 226, 226, 226, 110, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 0, 0, 226, 226, 226, 226, 0,
            0, 64, 0, 0, 0, 0, 0, 0, 22, 22, 22, 0, 0, 0, 0, 0, 0, 255, 246, 0, 0, 255, 255, 0, 0,
            0, 0, 49, 255, 255, 255, 158, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 85, 54, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 254, 56, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 255, 247, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 108, 0, 252, 18, 0,
            0, 0, 60, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 105, 0, 68, 93, 77, 77, 64, 77, 69, 77, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 56, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 246, 0, 0, 18, 255, 255, 1, 0, 0, 0, 0, 60, 0, 0,
            0, 0, 0, 7, 0, 0, 0, 0, 105, 0, 0, 0, 0, 0, 0, 0, 246, 255, 0, 0, 0, 85, 126, 68, 0, 0,
            0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 108, 0, 255, 18, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 7, 0, 0,
            0, 0, 105, 0, 68, 93, 77, 77, 17, 77, 69, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255,
            255, 56, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            246, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 254, 56, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            255, 247, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 108, 0, 252, 18, 0, 0, 0, 60, 0, 0, 0,
            0, 0, 0, 7, 0, 0, 0, 0, 105, 0, 68, 93, 77, 77, 16, 77, 69, 77, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 59, 255, 255, 255, 56, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 94, 0, 0, 0, 49, 36, 0, 0, 226,
            226, 226, 226, 226, 185, 226, 226, 226, 226, 226, 226, 226, 226, 226, 226, 226, 226,
            226, 226, 226, 226, 110, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 246, 0, 0, 18, 93, 255, 1,
            0, 0, 0, 0, 60, 0, 0, 0, 0, 255, 1, 0, 0, 239, 0, 0, 60, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0,
            105, 0, 0, 0, 0, 0, 0, 0, 246, 255, 0, 0, 0, 18, 255, 255, 1, 0, 21, 0, 0, 0, 60, 0, 0,
            60, 0, 0, 0, 0, 0, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88,
            88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88,
            88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88,
            88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88,
            88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88,
            88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 0, 7, 0, 0, 0, 0, 105,
            0, 68, 93, 77, 77, 17, 77, 69, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 56, 0,
            0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 105, 0, 68, 93, 77, 77, 17, 77, 69, 77, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 59, 255, 255, 255, 56, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 108, 0, 255, 18, 0, 0, 0, 60, 0, 59,
            0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 105, 0, 68, 93, 77, 77, 17, 77, 69, 77, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 59, 255, 255, 255, 56, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 246, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 254, 56, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 255, 247, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 108, 0, 252, 18, 0,
            0, 0, 60, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 105, 0, 68, 93, 77, 77, 16, 77, 69, 77, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 255, 56, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 246, 0, 0, 18, 255, 255, 1, 0, 0, 0, 0, 60, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 246, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255, 255, 254,
            56, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 247, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            7, 0, 0, 0, 0, 105, 0, 68, 93, 77, 77, 17, 77, 69, 77, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 245, 0, 0, 255, 0, 0, 59, 255, 255,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());

        let bytes = [
            85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 255, 255, 255, 19, 0, 0, 0, 0, 0, 0, 0, 4,
            0, 0, 0, 0, 0, 0, 0, 0, 255, 247, 0, 0, 0, 255, 112, 0, 0, 0, 0, 49, 255, 255, 255,
            143, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 1,
            0, 0, 20, 0, 0, 246, 255, 255, 255, 255, 0, 0, 255, 209, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 19, 0, 255, 255, 0, 134, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 246, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            255, 19, 0, 255, 255, 33, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 128, 255, 255,
            0, 0, 0, 255, 112, 0, 0, 0, 0, 0, 0, 0, 85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 255,
            255, 255, 56, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 254, 255, 255, 59, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 255, 255, 0, 0, 0, 0, 1, 0,
            0, 0, 0, 0, 0, 5, 0, 0, 0, 4, 0, 77, 69, 77, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 59, 255, 255, 255, 20, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());

        let bytes = [
            85, 51, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 255, 255, 255, 19, 0, 0, 0, 0, 0, 0, 0, 4,
            0, 0, 0, 0, 0, 0, 0, 0, 255, 247, 0, 0, 0, 255, 112, 0, 0, 0, 252, 49, 255, 255, 255,
            143, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 153, 1, 0, 0, 0, 0, 0, 0, 78, 0, 2, 0, 0, 0,
            1, 0, 0, 20, 0, 0, 246, 255, 255, 255, 255, 0, 0, 255, 209, 1, 0, 0, 0, 146, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 247, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 4, 0, 0, 0, 134,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0, 246, 255, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 255, 19, 0, 255, 255, 33, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 128,
            255, 247, 0, 0, 0, 255, 112, 0, 0, 0, 0, 0, 0, 0, 85, 54, 68, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 59, 255, 255, 255, 184, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 254, 255, 255, 59, 0, 0, 0, 1,
            0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0, 255, 112, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 65, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 246, 255, 255, 255, 201, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 254, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145,
            145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145,
            145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 145,
            145, 145, 145, 145, 145, 145, 145, 145, 145, 145, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 54, 54, 54, 0, 0, 246,
        ];
        let rv = u3d_parse(&bytes, &file_name);
        assert!(rv.is_ok());
    }
}
