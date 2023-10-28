pub mod basic_vert {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "resources/shaders/basic.vert",
    }
}

pub mod basic_frag {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "resources/shaders/basic.frag",
    }
}
