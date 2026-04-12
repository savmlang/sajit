use inkwell::{
  OptimizationLevel,
  context::Context,
  targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
  },
};
use object::{File, Object, ObjectSection};
use sajit::relocations::{RelocKind, Relocation};

fn main() {
  // Target::initialize_native(&InitializationConfig::default())
  //   .expect("Failed to find native target");

  Target::initialize_all(&InitializationConfig::default());

  let triple = TargetTriple::create("aarch64-unknown-linux-gnu");

  // let triple = TargetMachine::get_default_triple();
  let target = Target::from_triple(&triple).expect("Cannot build Target");

  let machine = {
    target
      .create_target_machine(
        &triple,
        "generic",
        "",
        OptimizationLevel::None,
        RelocMode::Static,
        CodeModel::Large,
      )
      .expect("Cannot create machine")
  };

  let context = Context::create();
  let module = context.create_module("my_simd_fn");
  let builder = context.create_builder();

  let function = {
    let arg0 = context.i64_type();

    context.i64_type().fn_type(&[arg0.into()], false)
  };

  let function = module.add_function("identity", function, None);

  // Build fn
  {
    let block = context.append_basic_block(function, "idompotency");
    let arg0 = function.get_nth_param(0).unwrap();

    builder.position_at_end(block);
    let externfn = {
      let ctx64 = context.i64_type();

      context.void_type().fn_type(&[ctx64.into()], false)
    };

    let externfn = module.add_function("call_ffi", externfn, None);
    externfn.set_call_conventions(0);

    // Build call
    {
      let fn_ptr_global = module.add_global(
        externfn.as_global_value().as_pointer_value().get_type(),
        None,
        "fn_ptr_storage",
      );
      fn_ptr_global.set_initializer(&externfn.as_global_value().as_pointer_value());
      fn_ptr_global.set_constant(true);

      fn_ptr_global.set_section(Some(".text"));

      let load_ptr = builder
        .build_load(
          externfn.as_global_value().as_pointer_value().get_type(),
          fn_ptr_global.as_pointer_value(),
          "load_fn_addr",
        )
        .unwrap();

      builder.build_indirect_call(
        externfn.get_type(),
        load_ptr.into_pointer_value(),
        &[arg0.into()],
        "call_ffi",
      );
    }
    // builder.build_call(externfn, &[arg0.into()], "call_ffi");

    builder
      .build_return(Some(&arg0))
      .expect("Unable to build return");
  }

  module.print_to_stderr();

  let object = machine
    .write_to_memory_buffer(&module, FileType::Object)
    .expect("Get Memory Buffer");

  let bytes = object.as_slice();

  println!("{bytes:?}");

  let (machinecode, relocs) = {
    let slice = object.as_slice();

    let file = File::parse(slice).expect("Parsed");

    let section = file.section_by_name(".text").unwrap();

    let output = section.data().unwrap();

    let mut relocs = vec![];

    for (offset, reloc) in section.relocations() {
      println!("OFFSET : {offset}");

      println!("KIND: {:?}", reloc.kind());

      relocs.push(Relocation {
        addend: 0,
        symbol_addr: 0,
        offset: offset as _,
        kind: RelocKind::Abs8,
      });
    }

    println!("{output:?}");

    (output, relocs)
  };
}
