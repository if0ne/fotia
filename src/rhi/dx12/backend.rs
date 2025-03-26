use std::{ffi::CString, path::Path};

use oxidx::dx::{
    self, IAdapter3, IBlob, IBlobExt, IDebug, IDebug1, IDebugExt, IDevice, IFactory4, IFactory6,
    features::{Architecture1Feature, Options3Feature, OptionsFeature},
};
use smallvec::SmallVec;
use tracing::{debug, error, info, warn};

use crate::rhi::{
    backend::{Api, DebugFlags, DeviceType, RenderDeviceId, RenderDeviceInfo},
    shader::{CompiledShader, ShaderDesc},
    types::ShaderType,
};

use super::device::DxDevice;

#[derive(Debug)]
pub struct DxBackend {
    factory: dx::Factory4,
    _debug: Option<dx::Debug1>,

    adapters: Vec<dx::Adapter3>,
    adapter_infos: Vec<RenderDeviceInfo>,
}

impl DxBackend {
    pub fn new(debug_flags: DebugFlags) -> Self {
        let flags = if !debug_flags.is_empty() {
            dx::FactoryCreationFlags::Debug
        } else {
            dx::FactoryCreationFlags::empty()
        };

        let factory = dx::create_factory4(flags).expect("failed to create DXGI factory");

        let debug = if debug_flags.contains(DebugFlags::CpuValidation) {
            let debug: dx::Debug1 = dx::create_debug()
                .expect("failed to create debug")
                .try_into()
                .expect("failed to fetch debug1");

            debug.enable_debug_layer();

            if debug_flags.contains(DebugFlags::GpuValidation) {
                debug.set_enable_gpu_based_validation(true);
            }

            debug.set_callback(Box::new(|_, severity, _, msg| match severity {
                dx::MessageSeverity::Corruption => error!("[D3D12 Validation] {}", msg),
                dx::MessageSeverity::Error => error!("[D3D12 Validation] {}", msg),
                dx::MessageSeverity::Warning => warn!("[D3D12 Validation] {}", msg),
                dx::MessageSeverity::Info => info!("[D3D12 Validation] {}", msg),
                dx::MessageSeverity::Message => debug!("[D3D12 Validation] {}", msg),
            }));

            Some(debug)
        } else {
            None
        };

        let mut gpus = SmallVec::<[_; 4]>::new();

        if let Ok(factory) = TryInto::<dx::Factory7>::try_into(factory.clone()) {
            debug!("Factory7 is supported");

            let mut i = 0;

            while let Ok(adapter) =
                factory.enum_adapters_by_gpu_preference(i, dx::GpuPreference::HighPerformance)
            {
                let Ok(desc) = adapter.get_desc1() else {
                    i += 1;
                    continue;
                };

                if let Ok(device) = dx::create_device(Some(&adapter), dx::FeatureLevel::Level11) {
                    let mut feature = OptionsFeature::default();
                    device
                        .check_feature_support(&mut feature)
                        .expect("failed to check options");

                    let mut hardware = Architecture1Feature::new(0);
                    device
                        .check_feature_support(&mut hardware)
                        .expect("failed to check options");

                    let mut feature3 = Options3Feature::default();
                    device
                        .check_feature_support(&mut feature3)
                        .expect("failed to check options");

                    let ty = if desc.flags().contains(dx::AdapterFlags::Sofware) {
                        DeviceType::Cpu
                    } else if hardware.uma() {
                        DeviceType::Integrated
                    } else {
                        DeviceType::Discrete
                    };

                    gpus.push((
                        adapter,
                        RenderDeviceInfo {
                            name: desc.description().trim_matches('\0').to_string(),
                            id: i as RenderDeviceId,
                            is_cross_adapter_texture_supported: feature
                                .cross_adapter_row_major_texture_supported(),
                            is_uma: hardware.uma(),
                            ty,
                            copy_timestamp_support: feature3
                                .copy_queue_timestamp_queries_supported(),
                        },
                    ));
                }

                i += 1;
            }
        } else {
            let mut i = 0;
            while let Ok(adapter) = factory.enum_adapters(i) {
                let Ok(desc) = adapter.get_desc1() else {
                    i += 1;
                    continue;
                };

                if let Ok(device) = dx::create_device(Some(&adapter), dx::FeatureLevel::Level11) {
                    let mut feature = OptionsFeature::default();
                    device
                        .check_feature_support(&mut feature)
                        .expect("failed to check options");

                    let mut hardware = Architecture1Feature::new(0);
                    device
                        .check_feature_support(&mut hardware)
                        .expect("failed to check options");

                    let mut feature3 = Options3Feature::default();
                    device
                        .check_feature_support(&mut feature3)
                        .expect("failed to check options");

                    let ty = if desc.flags().contains(dx::AdapterFlags::Sofware) {
                        DeviceType::Cpu
                    } else if hardware.uma() {
                        DeviceType::Integrated
                    } else {
                        DeviceType::Discrete
                    };

                    gpus.push((
                        adapter,
                        RenderDeviceInfo {
                            name: desc.description().trim_matches('\0').to_string(),
                            id: i,
                            is_cross_adapter_texture_supported: feature
                                .cross_adapter_row_major_texture_supported(),
                            is_uma: hardware.uma(),
                            ty,
                            copy_timestamp_support: feature3
                                .copy_queue_timestamp_queries_supported(),
                        },
                    ));
                }

                i += 1;
            }

            gpus.sort_by(|(l, _), (r, _)| {
                let descs = (
                    l.get_desc1().map(|d| d.vendor_id()),
                    r.get_desc1().map(|d| d.vendor_id()),
                );

                match descs {
                    (Ok(0x8086), Ok(0x8086)) => std::cmp::Ordering::Equal,
                    (Ok(0x8086), Ok(_)) => std::cmp::Ordering::Less,
                    (Ok(_), Ok(0x8086)) => std::cmp::Ordering::Greater,
                    (_, _) => std::cmp::Ordering::Equal,
                }
            });
        }

        let (adapters, adapter_infos): (Vec<_>, Vec<_>) = gpus.into_iter().unzip();

        adapter_infos
            .iter()
            .for_each(|a| info!("Found adapter: {:?}", a));

        Self {
            factory,
            _debug: debug,
            adapters,
            adapter_infos,
        }
    }
}

impl Api for DxBackend {
    type Device = DxDevice;

    fn enumerate_devices(&self) -> impl Iterator<Item = &RenderDeviceInfo> + '_ {
        self.adapter_infos.iter()
    }

    fn create_device(&self, index: RenderDeviceId) -> Self::Device {
        DxDevice::new(
            self.adapters[index].clone(),
            self.factory.clone(),
            self.adapter_infos[index].clone(),
        )
    }

    fn compile_shader<P: AsRef<Path>>(&self, desc: &ShaderDesc<'_, P>) -> CompiledShader {
        let target = match desc.ty {
            ShaderType::Vertex => c"vs_5_1",
            ShaderType::Pixel => c"ps_5_1",
        };

        let flags = if desc.debug {
            dx::COMPILE_DEBUG | dx::COMPILE_SKIP_OPT
        } else {
            0
        };

        let defines = desc
            .defines
            .iter()
            .map(|(k, v)| {
                (
                    CString::new(k.as_bytes()).expect("CString::new failed"),
                    CString::new(v.as_bytes()).expect("CString::new failed"),
                )
            })
            .collect::<SmallVec<[_; 4]>>();

        let defines = defines
            .iter()
            .map(|(k, v)| dx::ShaderMacro::new(k, v))
            .chain(std::iter::once(dx::ShaderMacro::default()))
            .collect::<SmallVec<[_; 4]>>();

        let entry_point = CString::new(desc.entry_point.as_bytes()).expect("CString::new failed");

        let raw = dx::Blob::compile_from_file(&desc.path, &defines, &entry_point, target, flags, 0)
            .expect("Failed to compile a shader");

        let mapped = raw.get_buffer_ptr::<u8>();

        let slice = unsafe { std::slice::from_raw_parts(mapped.as_ptr(), raw.get_buffer_size()) };

        let mut raw = vec![0; raw.get_buffer_size()];
        raw.clone_from_slice(slice);

        CompiledShader { raw, ty: desc.ty }
    }
}
