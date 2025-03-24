use std::sync::{Arc, atomic::Ordering};

use hecs::World;
use tracing::info;

use crate::{
    collections::handle::Handle,
    engine::camera::Camera,
    multi_gpu_renderer::{
        passes::{
            directional_light_pass::DirectionalLightPass,
            gamma_corr_pass::GammaCorrectionPass,
            gpass::GPass,
            m_csm::{MgpuState, MultiCascadedShadowMapsPass},
            zpass::ZPass,
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
};

pub struct MultiGpuShadows<D: RenderDevice> {
    pub ctx: Arc<ContextDual<D>>,
    pub zpass: ZPass<D>,
    pub csm: MultiCascadedShadowMapsPass<D>,
    pub gpass: GPass<D>,
    pub dir_pass: DirectionalLightPass<D>,
    pub final_pass: GammaCorrectionPass<D>,
}

impl<D: RenderDevice> MultiGpuShadows<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<ContextDual<D>>,
        extent: [u32; 2],
        psos: &PsoCollection<D>,
        frames_in_flight: usize,
    ) -> Self {
        let zpass = ZPass::new(Arc::clone(&rs), Arc::clone(&ctx.primary), extent, psos);
        let csm = MultiCascadedShadowMapsPass::new(
            Arc::clone(&rs),
            Arc::clone(&ctx),
            2048,
            0.5,
            psos,
            frames_in_flight,
        );

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
            frames_in_flight,
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
        let copy_texture = self.csm.copy_texture.load(Ordering::Acquire);
        let working_texture = self.csm.working_texture.load(Ordering::Acquire);

        if self.ctx.secondary.is_ready(CommandType::Graphics)
            && self.csm.states[working_texture] == MgpuState::WaitForWrite
        {
            self.csm.update(camera, light_dir, working_texture);

            self.ctx.call_secondary(|ctx| {
                let mut cmd = ctx.create_encoder(CommandType::Graphics);
                let timings = cmd.begin(ctx);
                info!("Secondary Timings: {:?}", timings);
                ctx.enqueue(cmd);

                self.csm.render(working_texture, world);

                let mut cmd = ctx.create_encoder(CommandType::Graphics);

                cmd.set_barriers(&[
                    Barrier::Texture(
                        self.csm.shared[working_texture],
                        ResourceState::CopySrc,
                        Subresource::Local(None),
                    ),
                    Barrier::Texture(
                        self.csm.shared[working_texture],
                        ResourceState::CopyDst,
                        Subresource::Shared,
                    ),
                ]);
                {
                    let encoder = cmd.transfer("Push CSM".into());
                    encoder.push_texture(self.csm.shared[working_texture]);
                }

                ctx.commit(cmd);

                self.csm.states[working_texture] =
                    MgpuState::WaitForCopy(ctx.submit(CommandType::Graphics));
            });
        }

        if let MgpuState::WaitForCopy(v) = self.csm.states[copy_texture] {
            if self.ctx.primary.is_ready(CommandType::Transfer)
                && self.ctx.secondary.is_ready_for(CommandType::Graphics, v)
            {
                self.csm.next_working_texture();

                self.ctx.call_primary(|ctx| {
                    let mut cmd = ctx.create_encoder(CommandType::Transfer);
                    let timings = cmd.begin(ctx);
                    info!("Copy Timings: {:?}", timings);

                    cmd.set_barriers(&[
                        Barrier::Texture(
                            self.csm.shared[copy_texture],
                            ResourceState::CopyDst,
                            Subresource::Local(None),
                        ),
                        Barrier::Texture(
                            self.csm.shared[copy_texture],
                            ResourceState::CopySrc,
                            Subresource::Shared,
                        ),
                    ]);
                    {
                        let encoder = cmd.transfer("Pull CSM".into());
                        encoder.pull_texture(self.csm.shared[copy_texture]);
                    }

                    ctx.commit(cmd);
                    self.csm.states[copy_texture] =
                        MgpuState::WaitForRead(ctx.submit(CommandType::Transfer))
                });
            }
        }

        self.zpass.render(globals, frame_idx, world);

        self.gpass.render(globals, frame_idx, world);

        let copy_texture = if let MgpuState::WaitForRead(v) = self.csm.states[copy_texture] {
            if self.ctx.primary.is_ready_for(CommandType::Transfer, v) {
                self.csm.next_copy_texture();
                self.csm.states[copy_texture] = MgpuState::WaitForWrite;
                copy_texture
            } else {
                if copy_texture == 0 {
                    self.csm.frames_in_flight - 1
                } else {
                    copy_texture - 1
                }
            }
        } else {
            if copy_texture == 0 {
                self.csm.frames_in_flight - 1
            } else {
                copy_texture - 1
            }
        };

        self.dir_pass.render(
            globals,
            self.csm.shared[copy_texture],
            self.csm.argument[copy_texture],
            frame_idx,
            copy_texture,
        );

        self.final_pass.render(swapchain_view);
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.zpass.resize(extent);
        self.gpass.resize(extent);
        self.dir_pass.resize(extent);
        self.final_pass.resize(extent);
    }
}
