use std::sync::Arc;

use hecs::World;
use tracing::info;

use crate::{
    TimingsInfo,
    collections::{handle::Handle, rwc_ring_buffer::RwcState},
    engine::camera::Camera,
    multi_gpu_renderer::{
        passes::{
            directional_light_pass::DirectionalLightPass, gamma_corr_pass::GammaCorrectionPass,
            gpass::GPass, m_csm::MultiCascadedShadowMapsPass, zpass::ZPass,
        },
        pso::PsoCollection,
    },
    ra::{
        command::{Barrier, RenderCommandContext, RenderCommandEncoder, TransferEncoder},
        context::{ContextDual, RenderDevice},
        resources::Texture,
        shader::ShaderArgument,
        system::RenderSystem,
    },
    rhi::{
        command::{CommandType, Subresource},
        types::ResourceState,
    },
    settings::RenderSettings,
};

pub struct MultiGpuShadows<D: RenderDevice> {
    pub ctx: Arc<ContextDual<D>>,
    pub zpass: ZPass<D>,
    pub csm: MultiCascadedShadowMapsPass<D>,
    pub gpass: GPass<D>,
    pub dir_pass: DirectionalLightPass<D>,
    pub final_pass: GammaCorrectionPass<D>,
    pub sender: Option<std::sync::mpsc::Sender<TimingsInfo>>,
}

impl<D: RenderDevice> MultiGpuShadows<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<ContextDual<D>>,
        extent: [u32; 2],
        psos: &PsoCollection<D>,
        settings: &RenderSettings,
        sender: Option<std::sync::mpsc::Sender<TimingsInfo>>,
    ) -> Self {
        let zpass = ZPass::new(Arc::clone(&rs), Arc::clone(&ctx.primary), extent, psos);
        let csm =
            MultiCascadedShadowMapsPass::new(Arc::clone(&rs), Arc::clone(&ctx), settings, psos);

        let gpass = GPass::new(
            Arc::clone(&rs),
            Arc::clone(&ctx.primary),
            extent,
            zpass.depth,
            psos,
        );

        let dir_pass = DirectionalLightPass::new(
            Arc::clone(&rs),
            Arc::clone(&ctx.primary),
            extent,
            gpass.diffuse_srv,
            gpass.normal_srv,
            gpass.material_srv,
            gpass.accum,
            settings.frames_in_flight,
            psos,
        );

        let final_pass = GammaCorrectionPass::new(
            Arc::clone(&rs),
            Arc::clone(&ctx.primary),
            psos,
            gpass.accum_srv,
            extent,
        );

        Self {
            ctx,
            zpass,
            csm,
            gpass,
            dir_pass,
            final_pass,
            sender,
        }
    }

    pub fn update(&mut self, _camera: &Camera, _light_dir: glam::Vec3, _frame_index: usize) {
        //self.csm.update(camera, light_dir, frame_index);
    }

    pub fn render(
        &mut self,
        world: &World,
        globals: Handle<ShaderArgument>,
        swapchain_view: Handle<Texture>,
        camera: &Camera,
        light_dir: glam::Vec3,
        frame_idx: usize,
    ) {
        if self.ctx.secondary.is_ready(CommandType::Graphics)
            && self.csm.shared.head_state() == RwcState::WaitForWrite
        {
            self.csm.update(camera, light_dir);

            self.ctx.call_secondary(|ctx| {
                let mut cmd = ctx.create_encoder(CommandType::Graphics);
                let timings = cmd.begin(ctx);

                if let Some(sdr) = &mut self.sender {
                    if let Some(timings) = timings {
                        sdr.send(TimingsInfo::SecondaryMultiGpu(timings))
                            .expect("failed to send");
                    }
                } else {
                    info!("Secondary Timings: {:?}", timings);
                }

                ctx.enqueue(cmd);

                self.csm.render(world);

                let mut cmd = ctx.create_encoder(CommandType::Graphics);

                cmd.set_barriers(&[
                    Barrier::Texture(
                        *self.csm.shared.head_data(),
                        ResourceState::CopySrc,
                        Subresource::Local(None),
                    ),
                    Barrier::Texture(
                        *self.csm.shared.head_data(),
                        ResourceState::CopyDst,
                        Subresource::Shared,
                    ),
                ]);
                {
                    let encoder = cmd.transfer("Push CSM".into());
                    encoder.push_texture(*self.csm.shared.head_data());
                }

                ctx.commit(cmd);

                self.csm
                    .shared
                    .update_head_state(RwcState::WaitForCopy(ctx.submit(CommandType::Graphics)));
            });
        }

        if let RwcState::WaitForCopy(v) = self.csm.shared.tail_state() {
            if self.ctx.primary.is_ready(CommandType::Transfer)
                && self.ctx.secondary.is_ready_for(CommandType::Graphics, v)
            {
                self.csm.shared.advance_head();

                self.ctx.call_primary(|ctx| {
                    let mut cmd = ctx.create_encoder(CommandType::Transfer);
                    let timings = cmd.begin(ctx);

                    if let Some(sdr) = &mut self.sender {
                        if let Some(timings) = timings {
                            sdr.send(TimingsInfo::PrimaryCopyMultiGpu(timings))
                                .expect("failed to send");
                        }
                    } else {
                        info!("Copy Timings: {:?}", timings);
                    }

                    cmd.set_barriers(&[
                        Barrier::Texture(
                            *self.csm.shared.tail_data(),
                            ResourceState::CopyDst,
                            Subresource::Local(None),
                        ),
                        Barrier::Texture(
                            *self.csm.shared.tail_data(),
                            ResourceState::CopySrc,
                            Subresource::Shared,
                        ),
                    ]);
                    {
                        let encoder = cmd.transfer("Pull CSM".into());
                        encoder.pull_texture(*self.csm.shared.tail_data());
                    }

                    ctx.commit(cmd);
                    self.csm.shared.update_tail_state(RwcState::WaitForRead(
                        ctx.submit(CommandType::Transfer),
                    ));
                });
            }
        }

        self.zpass.render(globals, frame_idx, world);

        self.gpass.render(globals, frame_idx, world);

        let copy_texture = if let RwcState::WaitForRead(v) = self.csm.shared.tail_state() {
            if self.ctx.primary.is_ready_for(CommandType::Transfer, v) {
                self.csm.shared.update_tail_state(RwcState::WaitForWrite);
                let csm = *self.csm.shared.tail_data();
                Some(csm)
            } else {
                None
            }
        } else {
            None
        };

        let (csm, idx) = match copy_texture {
            Some(texture) => {
                let idx = self.csm.shared.tail;
                self.csm.shared.advance_tail();
                (texture, idx)
            }
            None => {
                let idx = self.csm.shared.tip_index();
                let texture = *self.csm.shared.tip_data();
                (texture, idx)
            }
        };

        self.dir_pass
            .render(globals, csm, self.csm.argument[idx], frame_idx, idx);

        self.final_pass.render(swapchain_view);
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.zpass.resize(extent);
        self.gpass.resize(extent);
        self.dir_pass.resize(extent);
        self.final_pass.resize(extent);
    }
}
