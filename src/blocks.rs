// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

#![allow(unused, non_snake_case, non_upper_case_globals, nonstandard_style)]
use crate::compression::*;
use std::io::Write;
//use crate::u3d_common::U3dContextKind::cShading;
use crate::common::{Block, ParsingContext, U3dBlockKind};
use bitstream_io::{BitRead, BitReader, BitWrite};
use byteorder::{LittleEndian, ReadBytesExt};
use log::{debug, info, trace, warn};
use measure_time::debug_time;
use num_enum::TryFromPrimitive;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::fs::File;
use std::io::{BufWriter, Cursor, Error, SeekFrom};
use std::io::{Read, Result, Seek};
use std::path::Path;
use std::{fmt, io};

#[derive(Clone, Default)]
pub struct UString {
    pub value: String,
}
impl UString {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
    ) -> io::Result<Self> {
        let nb = read_u16(r)?;
        let data: Vec<u8> = (0..nb).map(|_| read_u8(r)).collect::<Result<_>>()?;
        let str = match std::str::from_utf8(&data[..]) {
            Ok(s) => s,
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        };
        let rv = Self {
            value: str.to_owned(),
        };
        Ok(rv)
    }
}
impl fmt::Display for UString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.value)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}
impl fmt::Debug for UString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.value)
    }
}

pub struct Uuid {
    pub a: u32,
    pub b: u16,
    pub c: u16,
    pub d: [u8; 8],
}
impl Uuid {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
    ) -> io::Result<Self> {
        let a = read_u32(r)?;
        let b = read_u16(r)?;
        let c = read_u16(r)?;
        let d: Vec<u8> = (0..8).map(|_| read_u8(r)).collect::<Result<_>>()?;
        Ok(Self {
            a,
            b,
            c,
            d: *d.as_array().unwrap(),
        })
    }
}
impl PartialEq for Uuid {
    fn eq(&self, other: &Self) -> bool {
        self.a == other.a && self.b == other.b && self.c == other.c && self.d == other.d
    }
}

pub struct ColorRGBAF32 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Default)]
pub struct Attrib {
    attrib: u32,
    key: UString,
    value_string: Option<UString>,
    value_bin: Option<Vec<u8>>,
}
#[derive(Debug, Default)]
pub struct MetaData {
    pub attribs: Vec<Attrib>,
}
impl MetaData {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
    ) -> std::io::Result<Self> {
        let kv_pair_count = read_u32(r)?;
        const MAX_NUM_ATTRIBS: u32 = 64;
        if kv_pair_count > MAX_NUM_ATTRIBS {
            return Err(std::io::Error::other("Too many attribs!"));
        }
        let mut attribs = Vec::with_capacity(kv_pair_count as usize);
        for i in 0..kv_pair_count {
            let attrib = read_u32(r)?;
            let key = UString::from_reader(r)?;
            let mut value_string: Option<UString> = None;
            if (attrib & 1) == 0 {
                value_string = Some(UString::from_reader(r)?);
            }
            let mut value_bin: Option<Vec<u8>> = None;
            if (attrib & 1) != 0 {
                let value_bin_size = read_u32(r)?;
                value_bin = Some(
                    (0..value_bin_size)
                        .map(|_| read_u8(r))
                        .collect::<Result<_>>()?,
                );
            }
            attribs.push(Attrib {
                attrib,
                key,
                value_string,
                value_bin,
            });
        }
        Ok(Self { attribs })
    }
}

#[derive(Debug)]
pub struct FileHeader {
    major_version: i16,
    minor_version: i16,
    no_compression: bool,
    units_scaling_factor: f64,
}
impl FileHeader {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        contexts: &mut U3dCompressionContexts,
    ) -> io::Result<Self> {
        // this is the only required block
        //let r = r.bytereader().unwrap();
        let major_version = read_i16(r)?;
        let minor_version = read_i16(r)?;
        let profile_identifier = read_u32(r)?;
        let mut no_compression = false;
        if (profile_identifier & 4) != 0 {
            no_compression = true;
        }
        contexts.no_compression = no_compression;
        let declaration_size = read_u32(r)?;
        let file_size = read_u64(r)?;
        let character_encoding = read_u32(r)?;
        let mut units_scaling_factor = 1.0;
        if profile_identifier & 8u32 != 0 {
            units_scaling_factor = read_f64(r)?;
        }
        info!("U3D file version {} {}", major_version, minor_version);
        // debug!(
        //         "{}: v{}.{} {} {} {} {} {}",
        //         k,
        //         major_version,
        //         minor_version,
        //         profile_identifier,
        //         declaration_size,
        //         file_size,
        //         character_encoding,
        //         units_scaling_factor
        //     );
        //            Ok(())

        // let major_version = r.read_i16::<LittleEndian>()?;
        // let minor_version = r.read_i16::<LittleEndian>()?;
        // let profile_identifier = r.read_u32::<LittleEndian>()?;
        // let declaration_size = r.read_u32::<LittleEndian>()?;
        // let file_size = r.read_u64::<LittleEndian>()?;
        // let character_encoding = r.read_u32::<LittleEndian>()?;
        // let mut units_scaling_factor = 1.0;
        // if profile_identifier & 8u32 != 0 {
        //     units_scaling_factor = r.read_f64::<LittleEndian>()?;
        // }
        Ok(Self {
            major_version,
            minor_version,
            no_compression,
            units_scaling_factor,
        })
    }
}

#[derive(Debug)]
pub struct FileReference {
    name: UString,
    attributes: u32,
    urls: Vec<UString>,
}
impl FileReference {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        contexts: &mut U3dCompressionContexts,
    ) -> io::Result<Self> {
        let name = UString::from_reader(r)?;
        let attributes = read_u32(r)?;
        if attributes & 1 != 0 {
            let x = f32::from_bits(read_u32(r)?);
            let y = f32::from_bits(read_u32(r)?);
            let z = f32::from_bits(read_u32(r)?);
            let radius = f32::from_bits(read_u32(r)?);
        }
        if attributes & 2 != 0 {
            let bbox_min = read_f32_3(r)?;
            let bbox_max = read_f32_3(r)?;
        }
        let url_count = read_u32(r)?;
        const MAX_NUM_FILE_REFERENCES: u32 = 64;
        if url_count > MAX_NUM_FILE_REFERENCES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_FILE_REFERENCES ({}) file references found ({})!",
                MAX_NUM_FILE_REFERENCES, url_count
            )));
        }
        let mut urls = Vec::with_capacity(url_count as usize);
        for i in 0..url_count {
            urls.push(UString::from_reader(r)?);
        }
        let filter_count = read_u32(r)?;
        //debug!("{}", scope_name);
        Ok(Self {
            name,
            attributes,
            urls,
        })
    }
}
impl fmt::Display for FileReference {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

pub struct LightResource {
    pub name: UString,
    pub attributes: u32,
    pub light_type: u8,
    pub color: ColorRGBAF32,
    pub attenuation: [f32; 3],
    pub spot_angle: Option<f32>,
    pub intensity: f32,
}

#[derive(Debug)]
pub struct ModifierChain {
    pub name: UString,
    pub kind: u32,
    pub attributes: u32,
    pub modifier_count: u32,
}
impl ModifierChain {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        ctx: &mut ParsingContext,
        //contexts: &mut U3dCompressionContexts
    ) -> io::Result<Self> {
        let name = UString::from_reader(r)?;
        let kind = read_u32(r)?;
        let attributes = read_u32(r)?;
        if attributes & 1 != 0 {
            let x = f32::from_bits(read_u32(r)?);
            let y = f32::from_bits(read_u32(r)?);
            let z = f32::from_bits(read_u32(r)?);
            let radius = f32::from_bits(read_u32(r)?);
        }
        if attributes & 2 != 0 {
            let bbox_min = read_f32_3(r)?;
            let bbox_max = read_f32_3(r)?;
        }
        //r.reader().unwrap().stream_position()
        while r.reader().unwrap().stream_position()? % 4 != 0 {
            let _ = read_u8(r)?;
        }
        assert_eq!(r.position_in_bits()? % 32, 0);
        let modifier_count = read_u32(r)?;
        const MAX_NUM_MODIFIERS: u32 = 512;
        if modifier_count > MAX_NUM_MODIFIERS {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_MODIFIERS ({}) modifiers found ({})!",
                MAX_NUM_MODIFIERS, modifier_count
            )));
        }
        let rv = Self {
            name,
            kind,
            attributes,
            modifier_count,
        };
        trace!("{:#?}", rv);

        let before = r.position_in_bits()?;
        let mut num_mb_proc = 0;
        for mb in 0..modifier_count {
            //loop {
            let mut binding = r.bytereader().unwrap();
            let mut inner = binding.reader();
            // let mut inner = BitReader::endian(
            //     r,
            //     r.phantom.into()
            // );

            let r = Block::from_reader(&mut inner);
            let b: Block;
            match r {
                Ok(block) => {
                    b = block;
                    num_mb_proc += 1;
                }
                Err(_why) => break,
            }
            let bt = U3dBlockKind::try_from(b.block_type);
            if !bt.is_ok() {
                debug!("\tmc inner: 0x{:08x}", b.block_type);
                continue;
            }
            let bt = bt.unwrap();
            debug!("\tmc inner: {}", bt);

            //let mut contexts: U3dCompressionContexts = Default::default();
            crate::common::parse_block(bt, 0, &b, ctx, false)?;
        }
        if modifier_count != num_mb_proc {
            return Err(std::io::Error::other(
                "mismatching modifier blocks processed",
            ));
        }
        let after = r.position_in_bits()?;

        //debug!("{} {} {} {}", name, kind, attributes, modifier_count);
        Ok(rv)
    }
}

#[derive(Clone, Debug)]
pub struct ShadingDescription {
    //attributes: u32,
    has_diffuse: bool,
    has_specular: bool,
    //has_normals: bool,
    texture_layer_count: u32,
    texcoords_dims: Vec<u32>,
}
impl ShadingDescription {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        contexts: &mut U3dCompressionContexts,
    ) -> io::Result<Self> {
        let attributes = read_u32(r)?;
        let has_diffuse = (attributes & 1) != 0;
        let has_specular = (attributes & 2) != 0;
        //let has_normals = ;
        let texture_layer_count = read_u32(r)?;
        const MAX_NUM_TEXTURE_LAYERS: u32 = 8;
        if texture_layer_count > MAX_NUM_TEXTURE_LAYERS {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_TEXTURE_LAYERS ({}) found ({})!",
                MAX_NUM_TEXTURE_LAYERS, texture_layer_count
            )));
        }
        let mut texcoords_dims = Vec::with_capacity(texture_layer_count as usize);
        for i in 0..texture_layer_count {
            let texcoord_dims = read_u32(r)?;
            texcoords_dims.push(texcoord_dims);
        }
        let orig_shading_id = read_u32(r)?;
        Ok(Self {
            has_diffuse,
            has_specular,
            texture_layer_count,
            texcoords_dims,
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct CLODMeshDeclaration {
    pub name: UString,
    pub chain_index: u32,
    pub exclude_normals: bool,

    pub face_count: u32,
    pub position_count: u32,
    pub normal_count: u32,
    pub diffuse_color_count: u32,
    pub specular_color_count: u32,
    pub texture_coordinates_count: u32,

    pub shadings: Vec<ShadingDescription>,
}
impl CLODMeshDeclaration {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        contexts: &mut U3dCompressionContexts,
    ) -> io::Result<Self> {
        let name = UString::from_reader(r)?;
        let chain_index = read_u32(r)?;

        let mesh_attributes = read_u32(r)?;
        let exclude_normals = (mesh_attributes & 1) != 0;

        let face_count = read_u32(r)?;
        let position_count = read_u32(r)?;
        let normal_count = read_u32(r)?;
        let diffuse_color_count = read_u32(r)?;
        let specular_color_count = read_u32(r)?;
        let texture_coordinates_count = read_u32(r)?;
        let shading_count = read_u32(r)?;
        const MAX_NUM_SHADINGS: u32 = 64;
        if shading_count > MAX_NUM_SHADINGS {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_SHADINGS ({}) found ({})!",
                MAX_NUM_SHADINGS, shading_count
            )));
        }
        let mut shadings = Vec::with_capacity(shading_count as usize);
        for i in 0..shading_count {
            let shading = ShadingDescription::from_reader(r, contexts)?;
            shadings.push(shading);
        }

        let min_resolution = read_u32(r)?;
        let final_max_resolution = read_u32(r)?;

        let pos_qual_factor = read_u32(r)?;
        let norm_qual_factor = read_u32(r)?;
        let texcoord_qual_factor = read_u32(r)?;

        let point_inv_quant = read_f32(r)?;
        let norm_inv_quant = read_f32(r)?;
        let texcoord_inv_quant = read_f32(r)?;
        let diff_inv_quant = read_f32(r)?;
        let spec_inv_quant = read_f32(r)?;

        let norm_crease = read_f32(r)?;
        let norm_upd = read_f32(r)?;
        let norm_tol = read_f32(r)?;

        let bone_count = read_u32(r)?;
        for i in 0..bone_count {
            // TODO
        }

        Ok(Self {
            name,
            chain_index,
            exclude_normals,
            face_count,
            position_count,
            normal_count,
            diffuse_color_count,
            specular_color_count,
            texture_coordinates_count,
            shadings,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Face {
    pub material: u32,
    pub position: [u32; 3],
    pub normal: [u32; 3],
    pub diffuse: [u32; 3],
    pub specular: [u32; 3],
    pub texcoord: Vec<[u32; 3]>,
}
#[derive(Debug)]
pub struct CLODBaseMeshContinuation {
    pub name: UString,
    pub chain_index: u32,
    // pub base_face_count: u32,
    // pub base_position_count: u32,
    // pub base_normal_count: u32,
    // pub base_diffuse_color_count: u32,
    // pub base_specular_color_count: u32,
    // pub base_texture_coordinates_count: u32,
    pub base_position: Vec<[f32; 3]>,
    pub base_normal: Vec<[f32; 3]>,
    pub base_diffuse_color: Vec<[f32; 4]>,
    pub base_specular_color: Vec<[f32; 4]>,
    pub base_texture_coordinates: Vec<[f32; 4]>,
    pub faces: Vec<Face>,
}
impl CLODBaseMeshContinuation {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        ctx: &mut ParsingContext,
        //contexts: &mut U3dCompressionContexts
    ) -> io::Result<Self> {
        ctx.contexts.decoder_reset();

        let name = UString::from_reader(r)?;
        debug!("\tCLODBaseMeshContinuation: {}", name);
        let chain_index = read_u32(r)?;
        let base_face_count = read_u32(r)?;
        const MAX_NUM_FACES: u32 = 1024 * 1024;
        if base_face_count > MAX_NUM_FACES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_FACES ({}) found ({})!",
                MAX_NUM_FACES, base_face_count
            )));
        }
        const MAX_NUM_VERTICES: u32 = 1024 * 1024;
        let base_position_count = read_u32(r)?;
        if base_position_count > MAX_NUM_VERTICES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_VERTICES ({}) found ({})!",
                MAX_NUM_VERTICES, base_position_count
            )));
        }
        let base_normal_count = read_u32(r)?;
        if base_normal_count > MAX_NUM_VERTICES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_VERTICES ({}) found ({})!",
                MAX_NUM_VERTICES, base_normal_count
            )));
        }
        let base_diffuse_color_count = read_u32(r)?;
        if base_diffuse_color_count > MAX_NUM_VERTICES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_VERTICES ({}) found ({})!",
                MAX_NUM_VERTICES, base_diffuse_color_count
            )));
        }
        let base_specular_color_count = read_u32(r)?;
        if base_specular_color_count > MAX_NUM_VERTICES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_VERTICES ({}) found ({})!",
                MAX_NUM_VERTICES, base_specular_color_count
            )));
        }
        let base_texture_coordinates_count = read_u32(r)?;
        if base_texture_coordinates_count > MAX_NUM_VERTICES {
            return Err(std::io::Error::other(format!(
                "More than MAX_NUM_VERTICES ({}) found ({})!",
                MAX_NUM_VERTICES, base_texture_coordinates_count
            )));
        }

        //debug!("{}: {} chain_index={}", k, name, chain_index);

        // let base_face_count = read_u32(&mut r)?;
        // let base_position_count = read_u32(&mut r)?;
        // let base_normal_count = read_u32(&mut r)?;
        // let base_diffuse_color_count = read_u32(&mut r)?;
        // let base_specular_color_count = read_u32(&mut r)?;
        // let base_texture_coordinates_count = read_u32(&mut r)?;
        let base_position: Vec<[f32; 3]> = (0..base_position_count)
            .map(|_| {
                let mut xyz: [f32; 3] = [0.0; 3];
                match read_f32_3(r) {
                    Ok(val) => {
                        xyz = val;
                        Ok(xyz)
                    }
                    Err(why) => Err(why),
                }
            })
            .collect::<Result<_>>()?;
        let base_normal: Vec<[f32; 3]> = (0..base_normal_count)
            .map(|_| {
                let mut xyz: [f32; 3] = [0.0; 3];
                match read_f32_3(r) {
                    Ok(val) => {
                        xyz = val;
                        Ok(xyz)
                    }
                    Err(why) => Err(why),
                }
            })
            .collect::<Result<_>>()?;
        let base_diffuse_color: Vec<[f32; 4]> = (0..base_diffuse_color_count)
            .map(|_| {
                let mut rgba: [f32; 4] = [0.0; 4];
                match read_f32_4(r) {
                    Ok(val) => {
                        rgba = val;
                        Ok(rgba)
                    }
                    Err(why) => Err(why),
                }
            })
            .collect::<Result<_>>()?;
        let base_specular_color: Vec<[f32; 4]> = (0..base_specular_color_count)
            .map(|_| {
                let mut rgba: [f32; 4] = [0.0; 4];
                match read_f32_4(r) {
                    Ok(val) => {
                        rgba = val;
                        Ok(rgba)
                    }
                    Err(why) => Err(why),
                }
            })
            .collect::<Result<_>>()?;
        let base_texture_coordinates: Vec<[f32; 4]> = (0..base_texture_coordinates_count)
            .map(|_| {
                let mut uvst: [f32; 4] = [0.0; 4];
                match read_f32_4(r) {
                    Ok(val) => {
                        uvst = val;
                        Ok(uvst)
                    }
                    Err(why) => Err(why),
                }
            })
            .collect::<Result<_>>()?;

        let decl = match ctx.find_mesh_decl(&name.value) {
            Some(d) => d,
            None => {
                return Err(std::io::Error::other(format!(
                    "No previous matching (\"{}\") CLODMeshDeclaration!",
                    name.value
                )));
            }
        };
        let mut faces: Vec<Face> = Vec::with_capacity(base_face_count as usize);
        faces.resize(base_face_count as usize, Default::default());
        //ctx.contexts.create_decoder_compression_context(U3dContextKind::cShading);
        for fi in faces.iter_mut().take(base_face_count as usize) {
            //println!("{}", fi);
            let material_id = ctx
                .contexts
                .read_compressed_u32(r, U3dContextKind::cShading.into())?;
            if material_id as usize >= decl.shadings.len() {
                return Err(std::io::Error::other("Invalid material id!"));
            }
            let material = &decl.shadings[material_id as usize];
            for i in 0..3usize {
                fi.material = material_id;
                let pos_index = ctx
                    .contexts
                    .read_compressed_u32(r, range(base_position_count))?;
                if pos_index >= decl.position_count {
                    return Err(std::io::Error::other("Invalid index!"));
                }
                fi.position[i] = pos_index;
                if !decl.exclude_normals {
                    let normal_index = ctx
                        .contexts
                        .read_compressed_u32(r, range(base_normal_count))?;
                    if normal_index >= decl.normal_count {
                        return Err(std::io::Error::other("Invalid index!"));
                    }
                    fi.normal[i] = normal_index;
                }
                if material.has_diffuse {
                    let diffuse_index = ctx
                        .contexts
                        .read_compressed_u32(r, range(base_diffuse_color_count))?;
                    if diffuse_index >= decl.diffuse_color_count {
                        return Err(std::io::Error::other("Invalid index!"));
                    }
                    fi.diffuse[i] = diffuse_index;
                }
                if material.has_specular {
                    let specular_index = ctx
                        .contexts
                        .read_compressed_u32(r, range(base_specular_color_count))?;
                    if specular_index >= decl.specular_color_count {
                        return Err(std::io::Error::other("Invalid index!"));
                    }
                    fi.specular[i] = specular_index;
                }
                let texture_layer_count = material.texture_layer_count;
                fi.texcoord
                    .resize(texture_layer_count as usize, Default::default());
                for j in 0..texture_layer_count {
                    // Texture Layer Count in the shading description indicated by Shading ID
                    // determines the number of times Base Texture Coord Index in repeated at this corner.
                    let texture_index = ctx
                        .contexts
                        .read_compressed_u32(r, range(base_texture_coordinates_count))?;
                    if texture_index >= decl.texture_coordinates_count {
                        return Err(std::io::Error::other("Invalid index!"));
                    }
                    fi.texcoord[j as usize][i] = texture_index;
                }
            }
        }
        ctx.contexts
            .release_decoder_compression_context(U3dContextKind::cShading);

        Ok(Self {
            name,
            chain_index,
            // base_face_count,
            // base_position_count,
            // base_normal_count,
            // base_diffuse_color_count,
            // base_specular_color_count,
            // base_texture_coordinates_count,
            base_position,
            base_normal,
            base_diffuse_color,
            base_specular_color,
            base_texture_coordinates,
            faces,
        })
    }

    pub fn save(&self, file_name: &str) -> io::Result<()> {
        let mut w = BufWriter::new(File::create(file_name)?);
        writeln!(
            &mut w,
            "# written by https://github.com/ralovich/u3d {}",
            crate::LIBU3D_VERSION
        )?;
        writeln!(&mut w, "# {}", self.name)?;
        for v in self.base_position.iter() {
            writeln!(&mut w, "v {} {} {}", v[0], v[1], v[2])?;
        }
        let has_normals = !self.base_normal.is_empty();
        if has_normals {
            for n in self.base_normal.iter() {
                writeln!(&mut w, "vn {} {} {}", n[0], n[1], n[2])?;
            }
        }
        for face in self.faces.iter() {
            if has_normals {
                writeln!(
                    &mut w,
                    "f {}//{} {}//{} {}//{}",
                    face.position[0] + 1,
                    face.normal[0] + 1,
                    face.position[1] + 1,
                    face.normal[1] + 1,
                    face.position[2] + 1,
                    face.normal[2] + 1
                )?;
            } else {
                writeln!(
                    &mut w,
                    "f {} {} {}",
                    face.position[0] + 1,
                    face.position[1] + 1,
                    face.position[2] + 1
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CLODProgressiveMeshContinuation {
    pub name: UString,
    pub chain_index: u32,
    pub start_resolution: u32,
    pub end_resolution: u32,
    // pub base_normal_count: u32,
    // pub base_diffuse_color_count: u32,
    // pub base_specular_color_count: u32,
    // pub base_texture_coordinates_count: u32,
    // pub base_position: Vec<[f32; 3]>,
    // pub base_normal: Vec<[f32; 3]>,
    // pub base_diffuse_color: Vec<[f32; 4]>,
    // pub base_specular_color: Vec<[f32; 4]>,
    // pub base_texture_coordinates: Vec<[f32; 4]>,
    // pub faces: Vec<Face>,
}
impl CLODProgressiveMeshContinuation {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        ctx: &mut ParsingContext,
        //contexts: &mut U3dCompressionContexts
    ) -> io::Result<Self> {
        ctx.contexts.decoder_reset();

        let name = UString::from_reader(r)?;
        debug!("\tCLODProgressiveMeshContinuation: {}", name);
        let chain_index = read_u32(r)?;
        let start_resolution = read_u32(r)?;
        let end_resolution = read_u32(r)?;
        //let base_normal_count = read_u32(r)?;
        //let base_diffuse_color_count = read_u32(r)?;
        //let base_specular_color_count = read_u32(r)?;
        //let base_texture_coordinates_count = read_u32(r)?;

        // let range;
        // if CurrentPositionCount==0 {
        //     range=cZero;
        // }
        // else {
        //     range = r0;
        // }
        // let split_position_index = ctx.contexts.u3d_get_static_compressed_u32(r, range)?;

        Ok(Self {
            name,
            chain_index,
            start_resolution,
            end_resolution,
        })
    }
}

#[derive(Default)]
pub struct RHAdobeMeshResource {}
impl RHAdobeMeshResource {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        ctx: &mut ParsingContext,
        //contexts: &mut U3dCompressionContexts
    ) -> io::Result<Self> {
        Ok(Self {})
    }
}

pub struct Color {
    r: f32,
    g: f32,
    b: f32,
}

#[derive(Default)]
pub struct MaterialResource {
    // name: UString,
    // attributes: u32,
    // ambient: Color,
    // diffuse: Color,
    // specular: Color,
    // emissive: Color,
    // reflectivity: f32,
    // opacity: f32,
}
impl MaterialResource {
    pub fn from_reader<R: std::io::Read + std::io::Seek, E: bitstream_io::Endianness>(
        r: &mut BitReader<R, E>,
        contexts: &mut U3dCompressionContexts,
    ) -> io::Result<Self> {
        Ok(Self {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy() {}
}
