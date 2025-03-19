use std::{borrow::Cow, collections::HashMap, time::Duration};

pub mod backend;
pub mod command;
pub mod context;
pub mod resources;
pub mod shader;
pub mod swapchain;
pub mod system;

mod container;

#[derive(Clone, Debug)]
pub struct Timings {
    pub timings: HashMap<Cow<'static, str>, Duration>,
    pub total: Duration,
}
