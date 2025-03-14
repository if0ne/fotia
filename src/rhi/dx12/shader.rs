use oxidx::dx::{self, IDevice};

use crate::rhi::shader::{BindingType, PipelineLayoutDesc, RasterPipelineDesc, RenderShaderDevice};

use super::{conv::map_static_sampler, device::DxDevice};

impl RenderShaderDevice for DxDevice {
    type PipelineLayout = DxPipelineLayout;
    type ShaderArgument = ();
    type RasterPipeline = ();

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
        todo!()
    }

    fn destroy_raster_pipeline(&self, pipeline: Self::RasterPipeline) {
        todo!()
    }
}

#[derive(Clone, Debug)]
pub struct DxPipelineLayout {
    pub(super) raw: dx::RootSignature,
}
