use std::path::Path;

use glam::{Mat4, Vec3, Vec4};
use tracing::info;

use crate::{
    collections::handle::Handle,
    ra::{
        resources::{Buffer, Texture},
        shader::ShaderArgument,
        system::RenderSystem,
    },
};

#[derive(Clone, Debug)]
pub enum ImageSource {
    Path(std::path::PathBuf),
    Data(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Material {
    pub diffuse_color: [f32; 4],
    pub fresnel_r0: f32,
    pub roughness: f32,
    pub diffuse_map: Option<usize>,
    pub normal_map: Option<usize>,
}

#[derive(Clone, Default, Debug)]
pub struct GltfScene {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub indices: Vec<u32>,

    pub sub_meshes: Vec<Submesh>,
    pub materials: Vec<Material>,
    pub images: Vec<ImageSource>,
}

#[derive(Clone, Debug)]
pub struct PreparedGltfScene {
    pub positions: Handle<Buffer>,
    pub normals: Handle<Buffer>,
    pub uvs: Handle<Buffer>,
    pub tangents: Handle<Buffer>,
    pub indices: Handle<Buffer>,

    pub submeshes: Vec<(Handle<Buffer>, Handle<ShaderArgument>)>,
    pub materials: Vec<(Handle<Buffer>, Handle<ShaderArgument>)>,
    pub images: Vec<Handle<Texture>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Submesh {
    pub index_count: u32,
    pub start_index_location: u32,
    pub base_vertex_location: u32,
    pub material_idx: usize,
}

fn iter_gltf_node_tree<F: FnMut(&gltf::scene::Node, Mat4)>(
    node: &gltf::scene::Node,
    xform: Mat4,
    f: &mut F,
) {
    let node_xform = Mat4::from_cols_array_2d(&node.transform().matrix());
    let xform = xform * node_xform;

    f(node, xform);
    for child in node.children() {
        iter_gltf_node_tree(&child, xform, f);
    }
}

impl GltfScene {
    pub fn load(path: impl AsRef<Path>) -> Self {
        let curr_dir = std::env::current_dir().expect("Failed to get current dir");

        info!("Loading scene: {:?}", path.as_ref());
        let (gltf, buffers, _) = gltf::import(&path).expect("Failed to open file");

        let scene = gltf
            .default_scene()
            .or_else(|| gltf.scenes().next())
            .expect("Failed to fetch scene");
        let mut res = GltfScene::default();

        res.images = gltf
            .images()
            .map(|image| match image.source() {
                gltf::image::Source::View { view, .. } => {
                    let buffer_data = &buffers[view.buffer().index()];
                    let start = view.offset();
                    let end = start + view.length();
                    ImageSource::Data(buffer_data[start..end].to_vec())
                }
                gltf::image::Source::Uri { uri, .. } => {
                    info!("Find texture by path: {:?}\\{}", curr_dir, uri);
                    let path = path
                        .as_ref()
                        .parent()
                        .unwrap_or_else(|| Path::new("./"))
                        .join(uri);
                    ImageSource::Path(path)
                }
            })
            .collect();

        res.materials = gltf
            .materials()
            .map(|m| Material {
                diffuse_color: m.pbr_metallic_roughness().base_color_factor(),
                fresnel_r0: m.pbr_metallic_roughness().metallic_factor(),
                roughness: m.pbr_metallic_roughness().roughness_factor(),
                diffuse_map: m
                    .pbr_metallic_roughness()
                    .base_color_texture()
                    .map(|t| t.texture().index()),
                normal_map: m.normal_texture().map(|m| m.texture().index()),
            })
            .collect();

        let mut process_node = |node: &gltf::scene::Node, xform: Mat4| {
            if let Some(mesh) = node.mesh() {
                let flip_winding_order = xform.determinant() < 0.0;

                for prim in mesh.primitives() {
                    let reader = prim.reader(|buffer| Some(&buffers[buffer.index()]));

                    let positions = if let Some(iter) = reader.read_positions() {
                        iter.collect::<Vec<_>>()
                    } else {
                        return;
                    };

                    let normals = if let Some(iter) = reader.read_normals() {
                        iter.collect::<Vec<_>>()
                    } else {
                        return;
                    };

                    let (mut tangents, tangents_found) = if let Some(iter) = reader.read_tangents()
                    {
                        (iter.collect::<Vec<_>>(), true)
                    } else {
                        (vec![[1.0, 0.0, 0.0, 0.0]; positions.len()], false)
                    };

                    let (mut uvs, uvs_found) = if let Some(iter) = reader.read_tex_coords(0) {
                        (iter.into_f32().collect::<Vec<_>>(), true)
                    } else {
                        (vec![[0.0, 0.0]; positions.len()], false)
                    };

                    let mut indices: Vec<u32>;
                    {
                        if let Some(indices_reader) = reader.read_indices() {
                            indices = indices_reader.into_u32().collect();
                        } else {
                            if positions.is_empty() {
                                return;
                            }

                            match prim.mode() {
                                gltf::mesh::Mode::Triangles => {
                                    indices = (0..positions.len() as u32).collect();
                                }
                                _ => {
                                    panic!("Primitive mode {:?} not supported yet", prim.mode());
                                }
                            }
                        }

                        if flip_winding_order {
                            for tri in indices.chunks_exact_mut(3) {
                                tri.swap(0, 2);
                            }
                        }
                    }

                    if !tangents_found && uvs_found {
                        mikktspace::generate_tangents(&mut TangentCalcContext {
                            indices: indices.as_slice(),
                            positions: positions.as_slice(),
                            normals: normals.as_slice(),
                            uvs: uvs.as_slice(),
                            tangents: tangents.as_mut_slice(),
                        });
                    }

                    let submesh = Submesh {
                        index_count: indices.len() as u32,
                        start_index_location: res.indices.len() as u32,
                        base_vertex_location: res.positions.len() as u32,
                        material_idx: prim.material().index().unwrap_or(0),
                    };

                    res.indices.append(&mut indices);

                    for v in positions {
                        let pos = (xform * Vec3::from(v).extend(1.0)).truncate();
                        res.positions.push(pos.into());
                    }

                    for v in normals {
                        let norm = (xform * Vec3::from(v).extend(0.0)).truncate().normalize();
                        res.normals.push(norm.into());
                    }

                    for v in tangents {
                        let v = Vec4::from(v);
                        let t = (xform * v.truncate().extend(0.0)).truncate().normalize();
                        res.tangents.push(
                            t.extend(v.w * if flip_winding_order { -1.0 } else { 1.0 })
                                .into(),
                        );
                    }

                    res.uvs.append(&mut uvs);

                    res.sub_meshes.push(submesh);
                }
            }
        };

        let xform = Mat4::IDENTITY;
        for node in scene.nodes() {
            iter_gltf_node_tree(&node, xform, &mut process_node);
        }

        res
    }

    pub fn prepare(&self, rs: &RenderSystem) -> PreparedGltfScene {
        PreparedGltfScene {
            positions: rs.create_buffer_handle(),
            normals: rs.create_buffer_handle(),
            uvs: rs.create_buffer_handle(),
            tangents: rs.create_buffer_handle(),
            indices: rs.create_buffer_handle(),
            submeshes: self
                .sub_meshes
                .iter()
                .map(|_| {
                    (
                        rs.create_buffer_handle(),
                        rs.create_shader_argument_handle(),
                    )
                })
                .collect(),
            materials: self
                .materials
                .iter()
                .map(|_| {
                    (
                        rs.create_buffer_handle(),
                        rs.create_shader_argument_handle(),
                    )
                })
                .collect(),
            images: self
                .images
                .iter()
                .map(|_| rs.create_texture_handle())
                .collect(),
        }
    }
}

struct TangentCalcContext<'a> {
    indices: &'a [u32],
    positions: &'a [[f32; 3]],
    normals: &'a [[f32; 3]],
    uvs: &'a [[f32; 2]],
    tangents: &'a mut [[f32; 4]],
}

impl<'a> mikktspace::Geometry for TangentCalcContext<'a> {
    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.indices[face * 3 + vert] as usize]
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.indices[face * 3 + vert] as usize]
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uvs[self.indices[face * 3 + vert] as usize]
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        self.tangents[self.indices[face * 3 + vert] as usize] = tangent;
    }
}
