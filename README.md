# Fotia

An experimental Multi-GPU renderer for master's thesis.

The first prototype can be found here [Multi-Gpu Shadows](https://github.com/if0ne/multi-gpu-shadows)

# How to start

1. Clone repo
2. Download any GLTF scene, such as https://skfb.ly/6C7pD
3. `cargo build --release`
4. Put the scene in the assets folder next to the generated executable file
5. Configure the path to the scene in config.toml, which lies next to the executable file (all settings [here](https://github.com/if0ne/fotia/blob/64309e8a4ef97a2ae800ccc7b41e4519d42487bd/src/settings.rs#L50))

# References

Inspired by:

1. [Halcyon by SEED](https://www.wihlidal.com/projects/seed-halcyon-1/)
2. [o3de](https://github.com/o3de/sig-graphics-audio/discussions/32)
3. [blade by kvark](https://github.com/kvark/blade)

# Demo

![Pica pica](./assets/demo.jpg)
