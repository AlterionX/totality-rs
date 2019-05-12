use super::{
    hal::{
        Backend, Device,
        pso::{Specialization, EntryPoint},
    },
};
use std::{
    mem::ManuallyDrop,
    borrow::Cow,
};
use shaderc::{CompileOptions, Compiler};

#[allow(dead_code)]
use log::{error, warn, info, debug, trace};

pub struct ShaderInfo<'a> {
    pub kind: shaderc::ShaderKind,
    pub name: &'a str,
    pub entry_fn: &'a str,
    pub src: &'a str,
    pub opts: Option<&'a CompileOptions<'a>>,
}
pub struct CompiledShader<'a, B: Backend> {
    module: ManuallyDrop<<B>::ShaderModule>,
    entry_fn: &'a str,
    dropped: bool,
}
impl <'a, B: Backend> CompiledShader<'a, B> {
    pub fn new(
        compiler: &mut Compiler,
        device: &mut <B>::Device,
        si: ShaderInfo<'a>,
    ) -> Result<CompiledShader<'a, B>, &'static str> {
        let ShaderInfo { kind, name, entry_fn, src, opts } = si;
        trace!("Compiling module {:?} with entry point {:?}.", name, entry_fn);
        let compiled_artifact = compiler.compile_into_spirv(
            src, kind,
            name, entry_fn,
            opts
        ).map_err(|e| {
            error!("Error compiling shader {}: {:?}", si.name, e);
            "Could not compile shader!"
        })?;
        let module = unsafe {
            device.create_shader_module(compiled_artifact.as_binary_u8())
                .map_err(|_| "Shader module creation failed!")?
        };
        let drop = ManuallyDrop::new(module);
        Ok(CompiledShader {
            module: drop,
            entry_fn: entry_fn,
            dropped: false
        })
    }
    pub fn get_entry_specialized(&'a self, sp: Specialization<'a>) -> EntryPoint<'a, B> {
        EntryPoint {
            entry: self.entry_fn,
            module: &self.module,
            specialization: sp,
        }
    }
    pub fn get_entry(&'a self) -> EntryPoint<'a, B> {
        self.get_entry_specialized(Specialization {
            constants: Cow::Borrowed(&[]),
            data: Cow::Borrowed(&[]),
        })
    }
    pub fn destroy(mut self, device: &mut <B>::Device) {
        trace!("Shader has been dumped!");
        if !self.dropped { unsafe {
            use std::ptr::read;
            device.destroy_shader_module(ManuallyDrop::into_inner(read(&mut self.module)));
            ManuallyDrop::drop(&mut self.module);
            self.dropped = true;
        } }
    }
}
impl <'a, B: Backend> Drop for CompiledShader<'a, B> {
    fn drop(&mut self) {
        if !self.dropped {
            panic!("Compiled shaders must be manually dropped!");
        }
    }
}

