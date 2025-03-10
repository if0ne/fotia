#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Unknown,

    Rgba8Unorm,

    R32,
    Rg32,
    Rgb32,
    Rgba32,
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
