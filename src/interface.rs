// -*- mode: rust; coding: utf-8-unix -*-

// SPDX-License-Identifier: MIT
//
// SPDX-FileCopyrightText: Copyright Kristóf Ralovich (C) 2025-2026.
// All rights reserved.

pub struct Face {
    pub indices: Vec<usize>,
}
pub struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    // colors
    // texcoords
    pub faces: Vec<Face>,
}
pub struct Scene {
    pub meshes: Vec<Mesh>,
    // textures
    // lights
    // cameras
}
