use crate::{
    ra::{backend::Backend, context::RenderDevice},
    rhi::{
        backend::Api,
        shader::{CompiledShader, ShaderDesc},
        types::ShaderType,
    },
    settings::RenderSettings,
};

pub struct ShaderCollection {
    pub csm: CompiledShader,
    pub csm_ps: CompiledShader,
    pub fullscreen: CompiledShader,
    pub directional_light_pass: CompiledShader,
    pub gamma_corr_pass: CompiledShader,
    pub zpass: CompiledShader,
    pub gpass_vs: CompiledShader,
    pub gpass_ps: CompiledShader,
}

impl ShaderCollection {
    pub fn new<A: Api<Device: RenderDevice>>(
        api: &Backend<A>,
        debug: bool,
        settings: &RenderSettings,
    ) -> Self {
        let csm = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: settings.asset_path.join("Csm.hlsl"),
            entry_point: "VSMain".into(),
            debug,
            defines: vec![],
        });

        let csm_ps = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: settings.asset_path.join("Csm.hlsl"),
            entry_point: "PSMain".into(),
            debug,
            defines: vec![],
        });

        let fullscreen = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: settings.asset_path.join("FullscreenVS.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let directional_light_pass = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: settings.asset_path.join("DirectionalLight.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![(
                "CASCADES_COUNT".into(),
                settings.cascades_count.to_string().into(),
            )],
        });

        let gamma_corr_pass = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: settings.asset_path.join("GammaCorr.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let zpass = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: settings.asset_path.join("Zpass.hlsl"),
            entry_point: "Main".into(),
            debug,
            defines: vec![],
        });

        let gpass_vs = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Vertex,
            path: settings.asset_path.join("GPass.hlsl"),
            entry_point: "VSMain".into(),
            debug,
            defines: vec![],
        });

        let gpass_ps = api.compile_shader(&ShaderDesc {
            ty: ShaderType::Pixel,
            path: settings.asset_path.join("GPass.hlsl"),
            entry_point: "PSMain".into(),
            debug,
            defines: vec![],
        });

        Self {
            csm,
            csm_ps,
            fullscreen,
            directional_light_pass,
            gamma_corr_pass,
            zpass,
            gpass_vs,
            gpass_ps,
        }
    }
}
