use std::sync::Arc;

use hecs::World;

use crate::{
    collections::handle::Handle,
    engine::camera::Camera,
    multi_gpu_renderer::{
        passes::{
            csm::CascadedShadowMapsPass, directional_light_pass::DirectionalLightPass,
            gamma_corr_pass::GammaCorrectionPass, gpass::GPass, zpass::ZPass,
        },
        pso::PsoCollection,
    },
    ra::{
        context::{Context, RenderDevice},
        resources::Texture,
        shader::ShaderArgument,
        system::RenderSystem,
    },
};

pub struct SingleGpuShadows<D: RenderDevice> {
    pub ctx: Arc<Context<D>>,
    pub zpass: ZPass<D>,
    pub csm: CascadedShadowMapsPass<D>,
    pub gpass: GPass<D>,
    pub dir_pass: DirectionalLightPass<D>,
    pub final_pass: GammaCorrectionPass<D>,
}

impl<D: RenderDevice> SingleGpuShadows<D> {
    pub fn new(
        rs: Arc<RenderSystem>,
        ctx: Arc<Context<D>>,
        extent: [u32; 2],
        psos: &PsoCollection<D>,
        frames_in_flight: usize,
    ) -> Self {
        let zpass = ZPass::new(Arc::clone(&rs), Arc::clone(&ctx), extent, psos);
        let csm = CascadedShadowMapsPass::new(
            Arc::clone(&rs),
            Arc::clone(&ctx),
            2048,
            0.5,
            psos,
            frames_in_flight,
        );

        let gpass = GPass::new(Arc::clone(&rs), Arc::clone(&ctx), extent, zpass.depth, psos);

        let dir_pass = DirectionalLightPass::new(
            Arc::clone(&rs),
            Arc::clone(&ctx),
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
            Arc::clone(&ctx),
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

    pub fn update(&mut self, camera: &Camera, light_dir: glam::Vec3, frame_index: usize) {
        self.csm.update(camera, light_dir, frame_index);
    }

    pub fn render(
        &self,
        world: &World,
        globals: Handle<ShaderArgument>,
        swapchain_view: Handle<Texture>,
        frame_idx: usize,
    ) {
        self.zpass.render(globals, frame_idx, world);

        self.csm.render(frame_idx, world);

        self.gpass.render(globals, frame_idx, world);

        self.dir_pass.render(
            globals,
            self.csm.srv,
            self.csm.argument,
            frame_idx,
            frame_idx,
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
