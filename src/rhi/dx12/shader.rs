use oxidx::dx::{self, IBlobExt, IDevice, IResource};

use crate::rhi::{
    dx12::conv::{map_cull_mode, map_depth_op, map_format, map_semantic, map_vertex_format},
    shader::{
        BindingType, PipelineLayoutDesc, RasterPipelineDesc, RenderShaderDevice,
        ShaderArgumentDesc, ShaderEntry,
    },
    types::ShaderType,
};

use super::{
    conv::map_static_sampler,
    device::{Descriptor, DxDevice},
};

impl RenderShaderDevice for DxDevice {
    type PipelineLayout = DxPipelineLayout;
    type ShaderArgument = DxShaderArgument;
    type RasterPipeline = DxRasterPipeline;

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
        desc: ShaderArgumentDesc<'_, '_, Self>,
    ) -> Self::ShaderArgument {
        let mut dynamic_index = 0;
        let views = if desc.views.len() > 0 {
            let size = self.descriptors.shader_heap.lock().inc_size;
            let views = self
                .descriptors
                .allocate(dx::DescriptorHeapType::CbvSrvUav, desc.views.len());

            for (i, view) in desc.views.iter().enumerate() {
                match view {
                    ShaderEntry::Cbv(buffer, buffer_size) => {
                        dynamic_index += 1;
                        self.gpu.create_constant_buffer_view(
                            Some(&dx::ConstantBufferViewDesc::new(
                                buffer.raw.get_gpu_virtual_address(),
                                *buffer_size,
                            )),
                            views.cpu.advance(i, size),
                        )
                    }
                    ShaderEntry::Srv(texture) | ShaderEntry::Uav(texture) => self
                        .create_texture_view(
                            views.cpu.advance(i, size),
                            &texture.raw,
                            &texture.view,
                            &texture.desc,
                        ),
                }
            }

            Some(views)
        } else {
            None
        };

        let samplers = if desc.samplers.len() > 0 {
            let size = self.descriptors.sampler_heap.lock().inc_size;
            let samplers = self
                .descriptors
                .allocate(dx::DescriptorHeapType::Sampler, desc.samplers.len());

            for (i, sampler) in desc.samplers.iter().enumerate() {
                self.gpu
                    .create_sampler(&sampler.desc, samplers.cpu.advance(i, size));
            }

            Some(samplers)
        } else {
            None
        };

        let dynamic_address = desc.dynamic_buffer.map(|b| b.raw.get_gpu_virtual_address());

        DxShaderArgument {
            views,
            samplers,
            dynamic_address,
            dynamic_index,
        }
    }

    fn destroy_shader_argument(&self, argument: Self::ShaderArgument) {
        if let Some(views) = argument.views {
            self.descriptors.shader_heap.lock().free(views);
        }

        if let Some(samplers) = argument.samplers {
            self.descriptors.sampler_heap.lock().free(samplers);
        }
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
                    s.ty,
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

        DxRasterPipeline {
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

#[derive(Debug)]
pub struct DxRasterPipeline {
    pub(super) raw: dx::PipelineState,
    pub(super) layout: Option<DxPipelineLayout>,
}

#[derive(Debug)]
pub struct DxShaderArgument {
    pub(super) views: Option<Descriptor>,
    pub(super) samplers: Option<Descriptor>,
    pub(super) dynamic_address: Option<u64>,
    pub(super) dynamic_index: u32,
}
