use oxidx::dx;

use crate::rhi::{
    command::CommandType,
    resources::{TextureDesc, TextureType, TextureUsages},
    types::Format,
};

pub(super) fn map_command_buffer_type(ty: CommandType) -> dx::CommandListType {
    match ty {
        CommandType::Graphics => dx::CommandListType::Direct,
        CommandType::Compute => dx::CommandListType::Compute,
        CommandType::Transfer => dx::CommandListType::Copy,
    }
}

pub(super) fn map_format(format: Format) -> dx::Format {
    match format {
        Format::Unknown => dx::Format::Unknown,
        Format::R32 => dx::Format::R32Float,
        Format::Rg32 => dx::Format::Rg32Float,
        Format::Rgb32 => dx::Format::Rgb32Float,
        Format::Rgba32 => dx::Format::Rgba32Float,
        Format::Rgba8Unorm => dx::Format::Rgba8Unorm,
        Format::Rgba8 => dx::Format::Rgba8Uint,
    }
}

pub(super) fn map_texture_desc(
    desc: &TextureDesc,
    is_cross_adapter_texture_supported: bool,
) -> dx::ResourceDesc {
    let raw_desc = match desc.ty {
        TextureType::D1 => dx::ResourceDesc::texture_1d(desc.extent[0]).with_array_size(1),
        TextureType::D1Array => {
            dx::ResourceDesc::texture_1d(desc.extent[0]).with_array_size(desc.extent[2] as u16)
        }
        TextureType::D2 => {
            dx::ResourceDesc::texture_2d(desc.extent[0], desc.extent[1]).with_array_size(1)
        }
        TextureType::D2Array => dx::ResourceDesc::texture_2d(desc.extent[0], desc.extent[1])
            .with_array_size(desc.extent[2] as u16),
        TextureType::D3 => {
            dx::ResourceDesc::texture_3d(desc.extent[0], desc.extent[1], desc.extent[2] as u16)
        }
    };

    raw_desc
        .with_alignment(dx::HeapAlignment::ResourcePlacement)
        .with_format(map_format(desc.format))
        .with_mip_levels(desc.mip_levels as u32)
        .with_layout(dx::TextureLayout::Unknown)
        .with_flags(map_texture_flags(
            desc.usage,
            is_cross_adapter_texture_supported,
        ))
}

pub(super) fn map_texture_flags(
    flags: TextureUsages,
    is_cross_adapter_texture_supported: bool,
) -> dx::ResourceFlags {
    let mut f = dx::ResourceFlags::empty();

    if flags.contains(TextureUsages::RenderTarget) && !flags.contains(TextureUsages::DepthTarget) {
        f |= dx::ResourceFlags::AllowRenderTarget;
    }

    if flags.contains(TextureUsages::DepthTarget) {
        f |= dx::ResourceFlags::AllowDepthStencil;

        if !flags.contains(TextureUsages::Resource) {
            f |= dx::ResourceFlags::DenyShaderResource;
        }
    }

    if flags.contains(TextureUsages::Storage) {
        f |= dx::ResourceFlags::AllowUnorderedAccess;
    }

    if flags.contains(TextureUsages::Shared)
        && !flags.contains(TextureUsages::DepthTarget)
        && is_cross_adapter_texture_supported
    {
        f |= dx::ResourceFlags::AllowCrossAdapter;
    }

    f
}
