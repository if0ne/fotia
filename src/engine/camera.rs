#[derive(Clone, Debug)]
pub struct Camera {
    pub view: glam::Mat4,
    pub far: f32,
    pub near: f32,
    pub fov: f32,
    pub aspect_ratio: f32,
}

impl Camera {
    pub fn proj(&self) -> glam::Mat4 {
        glam::Mat4::perspective_lh(self.fov, self.aspect_ratio, self.near, self.far)
    }
}
