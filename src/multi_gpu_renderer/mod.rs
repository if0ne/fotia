pub mod csm;
pub mod graphs;
pub mod passes;
pub mod pso;
pub mod shaders;

#[derive(Clone, Debug, Default)]
#[repr(C)]
#[repr(align(256))]
pub struct GpuGlobals {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub proj_view: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub inv_proj_view: glam::Mat4,

    pub eye_pos: glam::Vec3,
    pub _pad0: f32,

    pub screen_dim: glam::Vec2,
    pub _pad1: glam::Vec2,
}
