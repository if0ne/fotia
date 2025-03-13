#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Unknown,

    Rgba8Unorm,

    R32,
    Rg32,
    Rgb32,
    Rgba32,

    Rgba8,
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
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    #[default]
    Point,
    Linear,
    Anisotropic,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VertexFormat {
    Float,
    Float2,
    Float3,
    Float4,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VertexAttribute {
    Position(u8),
    Color(u8),
    Normal,
    Tangent,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InputElementDesc {
    pub semantic: VertexAttribute,
    pub format: VertexFormat,
    pub slot: u32,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DepthStateDesc {
    pub op: DepthOp,
    pub format: Format,
    pub read_only: bool,
}
