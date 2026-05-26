use std::mem::transmute;
use std::ops::Sub;

use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::targets::{
  CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::{AddressSpace, OptimizationLevel};
use sajit::relcar::RELCAR_BASIC;
use sajit::symbpool::LLVMSymbolPool;
use sajit::{
  LLVMDryRun, LLVMJITLink, MemoryExecutable, MemoryExecutableApi, MemorySizeInfo, WriteFnResult,
};

fn main() {
  let object = generate_objectfile();
  let symbpool = LLVMSymbolPool::new();

  let mut exec = MemoryExecutable::new_slab(None);

  let plt = (call as *const ()).addr().to_ne_bytes();
  let plt_addr = match exec.write_fn(&plt, &[], &RELCAR_BASIC) {
    WriteFnResult::Executable(e) => e,
    _ => unreachable!(),
  };

  let o = exec.cursor();
  let est = MemoryExecutable::sizecalc_jitlink(&symbpool, &object);
  let hmap = exec
    .write_jitlink(&symbpool, &object, |loc| {
      unsafe {
        println!("{}", &(*loc));
      }
      (plt_addr) as usize
    })
    .unwrap();
  let written = exec.cursor().sub(o);

  println!("Estimated : {est:?}, Found : {written}");

  println!("{hmap:?}");
  unsafe {
    let output: extern "C" fn() =
      transmute(*hmap.get("libcall").unwrap_or(hmap.get(".text").unwrap()));

    output();

    exec.leak();
  }
}

extern "C" fn call() {
  println!("Hello!, JIT Called me");
}

fn generate_objectfile() -> Vec<u8> {
  Target::initialize_native(&InitializationConfig::default())
    .expect("Failed to initialize native target");

  let ctx = Context::create();
  let module = ctx.create_module("ans");

  let void = ctx.void_type();
  let fn_type = void.fn_type(&[], false);

  {
    let fun = module.add_function("libcall2", fn_type, None);

    let block = ctx.append_basic_block(fun, "entry2");
    let builder = ctx.create_builder();
    builder.position_at_end(block);

    builder.build_return(None).unwrap();
  }

  // Define our function
  let fun = module.add_function("libcall", fn_type, None);

  let global_const = module.add_global(ctx.ptr_type(AddressSpace::default()), None, "MODFN");
  global_const.set_linkage(Linkage::DLLImport);

  // Declare an external function (this is the key part)
  // let ext = module.add_function("ext", fn_type, Some(Linkage::External));

  // Build function body
  let block = ctx.append_basic_block(fun, "entry");
  let builder = ctx.create_builder();
  builder.position_at_end(block);

  let ptr = global_const.as_pointer_value();
  let ptr_to_fn = builder
    .build_load(ctx.ptr_type(AddressSpace::default()), ptr, "indirectfn")
    .unwrap()
    .into_pointer_value();
  builder
    .build_indirect_call(fn_type, ptr_to_fn, &[], "name")
    .unwrap();
  // Call the external function
  // builder.build_call(ext, &[], "call_ext").unwrap();

  // Return void
  builder.build_return(None).unwrap();

  // Print IR (for sanity)
  module.print_to_stderr();

  // --- Create target machine ---
  let triple = TargetMachine::get_default_triple();
  let target = Target::from_triple(&triple).expect("Invalid target");

  let cpu = "generic";
  let features = "";

  let tm = target
    .create_target_machine(
      &triple,
      cpu,
      features,
      OptimizationLevel::Aggressive,
      RelocMode::Static,
      CodeModel::Medium,
    )
    .expect("Failed to create target machine");

  module.set_triple(&triple);
  module.set_data_layout(&tm.get_target_data().get_data_layout());

  // --- Emit object file to memory ---
  let buffer = tm
    .write_to_memory_buffer(&module, FileType::Object)
    .expect("Failed to emit object");

  let obj_bytes = buffer.as_slice();

  // 🔥 This is what you feed into your JITLink
  println!("Object size: {}", obj_bytes.len());

  obj_bytes.into()
}
