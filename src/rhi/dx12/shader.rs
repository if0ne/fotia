use oxidx::dx::{self, IBlobExt, IDevice};

use crate::rhi::{
    dx12::conv::{map_cull_mode, map_depth_op, map_format, map_semantic, map_vertex_format},
    shader::{BindingType, PipelineLayoutDesc, RasterPipelineDesc, RenderShaderDevice},
    types::ShaderType,
};

use super::{conv::map_static_sampler, device::DxDevice};

impl RenderShaderDevice for DxDevice {
    type PipelineLayout = DxPipelineLayout;
    type ShaderArgument = ();
    type RasterPipeline = DxRenderPipeline;

    fn create_pipeline_layout(&self, desc: PipelineLayoutDesc<'_>) -> Self::PipelineLayout {
        let mut ranges = vec![];
        let mut dynamic_buffers = vec![];

        for (i, set) in desc.sets.iter().enumerate() {
            let mut srvs = 0;
            let mut cbvs = 0;
            let mut uavs = 0;
            let mut samplers = 0;
            for binding in set.entries.iter() {
                let range = match binding.ty {
                    BindingType::Cbv => {
                        cbvs += 1;
                        dx::DescriptorRange::cbv(binding.nums, cbvs - 1)
                            .with_register_space(i as u32)
                    }
                    BindingType::Uav => {
                        uavs += 1;
                        dx::DescriptorRange::uav(binding.nums, uavs - 1)
                            .with_register_space(i as u32)
                    }
                    BindingType::Srv => {
                        srvs += 1;
                        dx::DescriptorRange::srv(binding.nums, srvs - 1)
                            .with_register_space(i as u32)
                    }
                    BindingType::Sampler => {
                        samplers += 1;
                        dx::DescriptorRange::sampler(binding.nums, samplers - 1)
                            .with_register_space(i as u32)
                    }
                };
                ranges.push(range);
            }

            if set.use_dynamic_buffer {
                dynamic_buffers.push((i, set.entries.len())); // set and index
            }
        }

        let mut tables = vec![];
        let mut offset = 0;

        for set in desc.sets {
            let ranges = &ranges[offset..(offset + set.entries.len())];

            if ranges.len() > 0 {
                tables.push(dx::RootParameter::descriptor_table(ranges));
                offset += set.entries.len();
            }
        }

        let parameters = dynamic_buffers
            .into_iter()
            .map(|(set, idx)| dx::RootParameter::cbv(idx as u32, set as u32))
            .chain(tables.into_iter())
            .collect::<Vec<_>>();

        let samplers = desc
            .static_samplers
            .iter()
            .map(|sampler| map_static_sampler(sampler))
            .collect::<Vec<_>>();

        let desc = dx::RootSignatureDesc::default()
            .with_parameters(&parameters)
            .with_samplers(&samplers)
            .with_flags(dx::RootSignatureFlags::AllowInputAssemblerInputLayout);

        let raw = self
            .gpu
            .serialize_and_create_root_signature(&desc, dx::RootSignatureVersion::V1_0, 0)
            .expect("failed to create pipeline layout");

        DxPipelineLayout { raw }
    }

    fn destroy_pipeline_layout(&self, _layout: Self::PipelineLayout) {}

    fn create_shader_argument(
        &self,
        desc: crate::rhi::shader::ShaderArgumentDesc<'_, '_, Self>,
    ) -> Self::ShaderArgument {
        todo!()
    }

    fn destroy_shader_argument(&self, argument: Self::ShaderArgument) {
        todo!()
    }

    fn create_raster_pipeline(&self, desc: RasterPipelineDesc<'_, Self>) -> Self::RasterPipeline {
        let input_element_desc = desc
            .input_elements
            .iter()
            .map(|el| {
                dx::InputElementDesc::per_vertex(
                    map_semantic(el.semantic),
                    map_vertex_format(el.format),
                    el.slot,
                )
            })
            .collect::<Vec<_>>();

        let raster = dx::RasterizerDesc::default()
            .with_fill_mode(dx::FillMode::Solid)
            .with_cull_mode(map_cull_mode(desc.cull_mode))
            .with_depth_bias(desc.depth_bias)
            .with_slope_scaled_depth_bias(desc.slope_bias);

        let raster = if desc.depth_clip {
            raster.enable_depth_clip()
        } else {
            raster
        };

        let vs = dx::Blob::from_bytes(&desc.vs.raw).expect("failed to create blob");

        let raw_desc = dx::GraphicsPipelineDesc::new(&vs)
            .with_input_layout(&input_element_desc)
            .with_blend_desc(
                dx::BlendDesc::default().with_render_targets(
                    desc.render_targets
                        .iter()
                        .map(|_| dx::RenderTargetBlendDesc::default()),
                ),
            )
            .with_render_targets(desc.render_targets.iter().map(|f| map_format(*f)))
            .with_rasterizer_state(raster)
            .with_primitive_topology(dx::PipelinePrimitiveTopology::Triangle);

        let mut raw_desc = if let Some(depth) = &desc.depth {
            raw_desc.with_depth_stencil(
                dx::DepthStencilDesc::default()
                    .enable_depth(map_depth_op(depth.op))
                    .with_depth_write_mask(if depth.read_only {
                        dx::DepthWriteMask::empty()
                    } else {
                        dx::DepthWriteMask::All
                    }),
                map_format(depth.format),
            )
        } else {
            raw_desc
        };

        let shaders = desc
            .shaders
            .iter()
            .map(|s| {
                (
                    dx::Blob::from_bytes(&s.raw).expect("failed to create blob"),
                    s.desc.ty,
                )
            })
            .collect::<Vec<_>>();

        for (shader, ty) in shaders.iter() {
            match ty {
                ShaderType::Pixel => raw_desc = raw_desc.with_ps(shader),
                ShaderType::Vertex => unreachable!(),
            }
        }

        let raw_desc = if let Some(layout) = &desc.layout {
            raw_desc.with_root_signature(&layout.raw)
        } else {
            raw_desc
        };

        let raw = self
            .gpu
            .create_graphics_pipeline(&raw_desc)
            .expect("failed to create pipeline");

        DxRenderPipeline {
            raw,
            layout: desc.layout.cloned(),
        }
    }

    fn destroy_raster_pipeline(&self, _pipeline: Self::RasterPipeline) {}
}

#[derive(Clone, Debug)]
pub struct DxPipelineLayout {
    pub(super) raw: dx::RootSignature,
}

#[derive(Clone, Debug)]
pub struct DxRenderPipeline {
    pub(super) raw: dx::PipelineState,
    pub(super) layout: Option<DxPipelineLayout>,
}
