use std::collections::HashMap;

use dolly::{prelude::*, rig::CameraRig};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug)]
pub struct Camera {
    pub far: f32,
    pub near: f32,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub rig: CameraRig<LeftHanded>,
}

impl Camera {
    pub fn new(
        near: f32,
        far: f32,
        fov: f32,
        extent: [u32; 2],
        rig: CameraRig<LeftHanded>,
    ) -> Self {
        Self {
            far,
            near,
            fov,
            aspect_ratio: extent[0] as f32 / extent[1] as f32,
            rig,
        }
    }

    pub fn proj(&self) -> glam::Mat4 {
        glam::Mat4::perspective_lh(self.fov, self.aspect_ratio, self.near, self.far)
    }

    pub fn view(&self) -> glam::Mat4 {
        glam::Mat4::from_rotation_translation(
            self.rig.final_transform.rotation.into(),
            self.rig.final_transform.position.into(),
        )
    }

    pub fn resize(&mut self, extent: [u32; 2]) {
        self.aspect_ratio = extent[0] as f32 / extent[1] as f32;
    }

    pub fn update(&mut self, keys: &HashMap<PhysicalKey, bool>, dt: f32) {
        let mut direction = glam::Vec3::ZERO;

        if keys
            .get(&PhysicalKey::Code(KeyCode::KeyW))
            .is_some_and(|v| *v)
        {
            direction.z += 1.0;
        }

        if keys
            .get(&PhysicalKey::Code(KeyCode::KeyS))
            .is_some_and(|v| *v)
        {
            direction.z -= 1.0;
        }

        if keys
            .get(&PhysicalKey::Code(KeyCode::KeyD))
            .is_some_and(|v| *v)
        {
            direction.x += 1.0;
        }

        if keys
            .get(&PhysicalKey::Code(KeyCode::KeyA))
            .is_some_and(|v| *v)
        {
            direction.x -= 1.0;
        }

        direction = direction.normalize();

        if direction.length() > f32::EPSILON {
            let quat: glam::Quat = self.rig.final_transform.rotation.into();

            self.rig
                .driver_mut::<dolly::prelude::Position>()
                .translate(quat * direction * dt * 10.0);

            self.rig.update(dt);
        }
    }

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.rig
            .driver_mut::<YawPitch>()
            .rotate_yaw_pitch(-0.3 * dx as f32, -0.3 * dy as f32);
    }
}
