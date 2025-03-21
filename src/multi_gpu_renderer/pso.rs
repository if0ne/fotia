use std::sync::Arc;

use crate::{
    collections::handle::Handle,
    ra::{
        context::{ContextDual, RenderDevice},
        shader::{RasterPipeline, RasterPipelineDesc, RenderShaderContext},
        system::RenderSystem,
    },
    rhi::{
        shader::{
            BindingEntry, BindingSet, BindingType, PipelineLayoutDesc, SamplerType, StaticSampler,
        },
        types::{
            AddressMode, ComparisonFunc, CullMode, DepthOp, DepthStateDesc, Filter, Format,
            InputElementDesc, VertexAttribute, VertexType,
        },
    },
};

use super::shaders::ShaderCollection;

pub struct PsoCollection<D: RenderDevice> {
    rs: Arc<RenderSystem>,
    group: Arc<ContextDual<D>>,
    pub zpass: Handle<RasterPipeline>,
    pub csm_pass: Handle<RasterPipeline>,
    pub directional_light_pass: Handle<RasterPipeline>,
    pub gamma_corr_pass: Handle<RasterPipeline>,
    pub g_pass: Handle<RasterPipeline>,
}

impl<D: RenderDevice> PsoCollection<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        group: Arc<ContextDual<D>>,
        shaders: &ShaderCollection,
    ) -> Self {
        let zpass = rs.create_raster_pipeline_handle();
        let csm_pass = rs.create_raster_pipeline_handle();
        let directional_light_pass = rs.create_raster_pipeline_handle();
        let gamma_corr_pass = rs.create_raster_pipeline_handle();
        let g_pass = rs.create_raster_pipeline_handle();

        group.parallel(|ctx| {
            // ZPass
            let zpass_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                zpass_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[],
                },
            );

            ctx.bind_raster_pipeline(
                zpass,
                RasterPipelineDesc {
                    layout: Some(zpass_layout),
                    input_elements: &[InputElementDesc {
                        semantic: VertexAttribute::Position(0),
                        format: VertexType::Float3,
                    }],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: true,
                    depth: Some(DepthStateDesc {
                        op: DepthOp::LessEqual,
                        format: Format::D24S8,
                        read_only: false,
                    }),
                    render_targets: &[],
                    cull_mode: CullMode::Back,
                    vs: &shaders.zpass,
                    shaders: &[],
                },
            );

            rs.free_pipeline_layout_handle(zpass_layout);

            // CSM Pass
            let csm_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                csm_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[],
                },
            );

            ctx.bind_raster_pipeline(
                csm_pass,
                RasterPipelineDesc {
                    layout: Some(csm_layout),
                    input_elements: &[InputElementDesc {
                        semantic: VertexAttribute::Position(0),
                        format: VertexType::Float3,
                    }],
                    depth_bias: 10000,
                    slope_bias: 5.0,
                    depth_clip: false,
                    depth: Some(DepthStateDesc {
                        op: DepthOp::LessEqual,
                        format: Format::D32,
                        read_only: false,
                    }),
                    render_targets: &[],
                    cull_mode: CullMode::Back,
                    vs: &shaders.csm,
                    shaders: &[],
                },
            );

            rs.free_pipeline_layout_handle(csm_layout);

            // Directional Light Pass
            let directional_light_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                directional_light_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                            ],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[StaticSampler {
                        ty: SamplerType::Comparasion(ComparisonFunc::LessEqual),
                        address_mode: AddressMode::Clamp,
                    }],
                },
            );

            ctx.bind_raster_pipeline(
                directional_light_pass,
                RasterPipelineDesc {
                    layout: Some(directional_light_layout),
                    input_elements: &[
                        InputElementDesc {
                            semantic: VertexAttribute::Position(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Uv(0),
                            format: VertexType::Float3,
                        },
                    ],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: false,
                    depth: None,
                    render_targets: &[Format::Rgba32],
                    cull_mode: CullMode::None,
                    vs: &shaders.fullscreen,
                    shaders: &[&shaders.directional_light_pass],
                },
            );

            rs.free_pipeline_layout_handle(directional_light_layout);

            // Gamme Correction Pass
            let gamme_corr_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                gamme_corr_layout,
                PipelineLayoutDesc {
                    sets: &[BindingSet {
                        entries: &[BindingEntry::new(BindingType::Srv, 1)],
                        use_dynamic_buffer: true,
                    }],
                    static_samplers: &[StaticSampler {
                        ty: SamplerType::Sample(Filter::Linear),
                        address_mode: AddressMode::Clamp,
                    }],
                },
            );

            ctx.bind_raster_pipeline(
                gamma_corr_pass,
                RasterPipelineDesc {
                    layout: Some(gamme_corr_layout),
                    input_elements: &[
                        InputElementDesc {
                            semantic: VertexAttribute::Position(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Uv(0),
                            format: VertexType::Float3,
                        },
                    ],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: false,
                    depth: None,
                    render_targets: &[Format::Rgba8Unorm],
                    cull_mode: CullMode::None,
                    vs: &shaders.fullscreen,
                    shaders: &[&shaders.gamma_corr_pass],
                },
            );

            rs.free_pipeline_layout_handle(gamme_corr_layout);

            // G Pass
            let gpass_layout = rs.create_pipeline_layout_handle();

            ctx.bind_pipeline_layout(
                gpass_layout,
                PipelineLayoutDesc {
                    sets: &[
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[
                                BindingEntry::new(BindingType::Srv, 1),
                                BindingEntry::new(BindingType::Srv, 1),
                            ],
                            use_dynamic_buffer: true,
                        },
                        BindingSet {
                            entries: &[],
                            use_dynamic_buffer: true,
                        },
                    ],
                    static_samplers: &[StaticSampler {
                        ty: SamplerType::Sample(Filter::Linear),
                        address_mode: AddressMode::Clamp,
                    }],
                },
            );

            ctx.bind_raster_pipeline(
                g_pass,
                RasterPipelineDesc {
                    layout: Some(gpass_layout),
                    input_elements: &[
                        InputElementDesc {
                            semantic: VertexAttribute::Position(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Normal(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Uv(0),
                            format: VertexType::Float3,
                        },
                        InputElementDesc {
                            semantic: VertexAttribute::Tangent(0),
                            format: VertexType::Float4,
                        },
                    ],
                    depth_bias: 0,
                    slope_bias: 0.0,
                    depth_clip: true,
                    depth: Some(DepthStateDesc {
                        op: DepthOp::Equal,
                        format: Format::D24S8,
                        read_only: true,
                    }),
                    render_targets: &[Format::Rgba32, Format::Rgba32, Format::Rgba32],
                    cull_mode: CullMode::Back,
                    vs: &shaders.gpass_vs,
                    shaders: &[&shaders.gpass_ps],
                },
            );

            rs.free_pipeline_layout_handle(gpass_layout);
        });

        Self {
            rs,
            group,
            zpass,
            csm_pass,
            directional_light_pass,
            gamma_corr_pass,
            g_pass,
        }
    }
}

impl<D: RenderDevice> Drop for PsoCollection<D> {
    fn drop(&mut self) {
        self.group.parallel(|ctx| {
            ctx.unbind_raster_pipeline(self.zpass);
            ctx.unbind_raster_pipeline(self.gamma_corr_pass);
            ctx.unbind_raster_pipeline(self.g_pass);
            ctx.unbind_raster_pipeline(self.directional_light_pass);
            ctx.unbind_raster_pipeline(self.csm_pass);
        });

        self.rs.free_raster_pipeline_handle(self.zpass);
        self.rs.free_raster_pipeline_handle(self.gamma_corr_pass);
        self.rs.free_raster_pipeline_handle(self.g_pass);
        self.rs
            .free_raster_pipeline_handle(self.directional_light_pass);
        self.rs.free_raster_pipeline_handle(self.csm_pass);
    }
}
