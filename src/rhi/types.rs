use std::{borrow::Cow, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Unknown,

    Rgba8Unorm,

    R32,
    Rg32,
    Rgb32,
    Rgba32,

    Rgba8,

    D24S8,
    D32,
}

impl Format {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Format::Unknown => 0,
            Format::Rgba8Unorm => 4,
            Format::R32 => 4,
            Format::Rg32 => 8,
            Format::Rgb32 => 12,
            Format::Rgba32 => 16,
            Format::Rgba8 => 4,
            Format::D24S8 => 4,
            Format::D32 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    #[default]
    Point,
    Linear,
    Anisotropic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComparisonFunc {
    None,
    Never,
    LessEqual,
    Equal,
    GreaterEqual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AddressMode {
    #[default]
    Wrap,
    Mirror,
    Clamp,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum CullMode {
    None,
    Back,
    Front,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum DepthOp {
    None,
    Less,
    Equal,
    LessEqual,
    Greater,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ShaderType {
    Vertex,
    Pixel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexType {
    Float,
    Float2,
    Float3,
    Float4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexAttribute {
    Position(u8),
    Uv(u8),
    Color(u8),
    Normal(u8),
    Tangent(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InputElementDesc {
    pub semantic: VertexAttribute,
    pub format: VertexType,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DepthStateDesc {
    pub op: DepthOp,
    pub format: Format,
    pub read_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scissor {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum IndexType {
    U16,
    U32,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ResourceState {
    Common,
    RenderTarget,
    Present,
    DepthWrite,
    DepthRead,
    Shader,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum GeomTopology {
    Triangles,
    Lines,
}

#[derive(Clone, Debug)]
pub struct Timings {
    pub timings: Vec<(Cow<'static, str>, Duration)>,
    pub total: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClearColor {
    Color([f32; 4]),
    Depth(f32),
}
