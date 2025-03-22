use glam::Vec4Swizzles;

use crate::engine::camera::Camera;

#[repr(C)]
#[repr(align(256))]
#[derive(Clone, Debug)]
pub struct Cascades {
    pub cascade_proj_views: [glam::Mat4; 4],
    pub distances: [f32; 4],
}

#[repr(C)]
#[repr(align(256))]
#[derive(Clone, Debug)]
pub struct Cascade {
    proj_view: glam::Mat4,
}

#[derive(Debug)]
pub struct CascadedShadowMaps {
    pub cascades: Cascades,
    pub lambda: f32,
}

impl CascadedShadowMaps {
    pub fn new(lambda: f32) -> Self {
        Self {
            cascades: Cascades {
                cascade_proj_views: [glam::Mat4::IDENTITY; 4],
                distances: [0.0; 4],
            },
            lambda,
        }
    }

    pub fn update(&mut self, camera: &Camera, light_dir: glam::Vec3) {
        let cascade_count = self.cascades.distances.len();

        let near_clip = camera.near;
        let far_clip = camera.far;
        let clip_range = far_clip - near_clip;

        let min_z = near_clip;
        let max_z = near_clip + clip_range;

        let range = max_z - min_z;
        let ratio: f32 = max_z / min_z;

        for (i, distance) in self.cascades.distances.iter_mut().enumerate() {
            let p = (i as f32 + 1.0) / cascade_count as f32;
            let log = min_z * ratio.powf(p);
            let uniform = min_z + range * p;
            *distance = self.lambda * (log - uniform) + uniform;
        }

        let mut cur_near = camera.near;

        for i in 0..cascade_count {
            let cur_far = self.cascades.distances[i];

            let mut corners = [
                glam::vec3(-1.0, -1.0, 0.0),
                glam::vec3(-1.0, -1.0, 1.0),
                glam::vec3(-1.0, 1.0, 0.0),
                glam::vec3(-1.0, 1.0, 1.0),
                glam::vec3(1.0, -1.0, 0.0),
                glam::vec3(1.0, -1.0, 1.0),
                glam::vec3(1.0, 1.0, 0.0),
                glam::vec3(1.0, 1.0, 1.0),
            ];

            let frust_proj =
                glam::Mat4::perspective_lh(camera.fov, camera.aspect_ratio, cur_near, cur_far);
            let cam_view = camera.view;

            let frust_proj_view = (frust_proj * cam_view).inverse();

            for corner in corners.iter_mut() {
                let temp = frust_proj_view * glam::vec4(corner.x, corner.y, corner.z, 1.0);
                let temp = temp / temp.w;

                *corner = temp.xyz();
            }

            let center = corners
                .into_iter()
                .fold(glam::Vec3::ZERO, |center, corner| center + corner)
                / 8.0;

            let light_view = glam::Mat4::look_at_lh(center, center + light_dir, glam::Vec3::Y);

            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;

            for corner in corners {
                let temp = light_view * glam::vec4(corner.x, corner.y, corner.z, 1.0);

                min_x = min_x.min(temp.x);
                max_x = max_x.max(temp.x);
                min_y = min_y.min(temp.y);
                max_y = max_y.max(temp.y);
                min_z = min_z.min(temp.z);
                max_z = max_z.max(temp.z);
            }

            let light_proj = glam::Mat4::orthographic_lh(min_x, max_x, min_y, max_y, min_z, max_z);

            self.cascades.cascade_proj_views[i] = light_proj * light_view;

            cur_near = cur_far;
        }
    }
}
