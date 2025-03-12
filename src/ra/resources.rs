use parking_lot::Mutex;

use crate::{collections::sparse_array::SparseArray, rhi::resources::RenderResourceDevice};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Buffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Texture;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sampler;

pub(super) struct ResourceMapper<D: RenderResourceDevice> {
    pub(super) buffers: Mutex<SparseArray<Buffer, D::Buffer>>,
    pub(super) textures: Mutex<SparseArray<Texture, D::Texture>>,
    pub(super) sampler: Mutex<SparseArray<Sampler, D::Sampler>>,
}

impl<D: RenderResourceDevice> Default for ResourceMapper<D> {
    fn default() -> Self {
        Self {
            buffers: Mutex::new(SparseArray::new(128)),
            textures: Mutex::new(SparseArray::new(128)),
            sampler: Mutex::new(SparseArray::new(128)),
        }
    }
}
