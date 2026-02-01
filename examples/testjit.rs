use std::mem::transmute;

use cranelift::{
  codegen::{
    Context,
    binemit::Reloc,
    ir::{Function, UserExternalName, UserFuncName},
  },
  native::builder_with_options,
  prelude::{settings::Flags, types::I64, *},
};

use sajit::{
  MemoryExecutable,
  relocations::{RelocKind, Relocation},
};

fn main() {
  let builder = settings::builder();

  let isa = builder_with_options(true)
    .expect("Unsupported target isa")
    .finish(Flags::new(builder))
    .expect("Could not build ISA");

  let mut sig = Signature::new(isa.default_call_conv());

  sig.params.push(AbiParam::new(I64));
  sig.params.push(AbiParam::new(I64));
  sig.returns.push(AbiParam::new(I64));

  // IR Building
  let mut func =
    Function::with_name_signature(UserFuncName::User(UserExternalName::new(0, 0)), sig.clone());
  let mut func_ctx = FunctionBuilderContext::new();

  {
    let mut builder = FunctionBuilder::new(&mut func, &mut func_ctx);

    let mut external_sig = Signature::new(isa.default_call_conv());
    external_sig.params.push(AbiParam::new(types::I64));
    // external_sig.returns.push(AbiParam::new(types::I64)); // Pointer as i64

    let si = builder.import_signature(external_sig);

    let user_name = UserExternalName::new(1, 0);
    let name_ref = builder.func.declare_imported_user_function(user_name);
    let fnref = builder.import_function(ExtFuncData {
      patchable: false,
      name: ExternalName::User(name_ref),
      signature: si,
      colocated: false,
    });

    let block = builder.create_block();
    builder.switch_to_block(block);
    builder.append_block_params_for_function_params(block);

    let arg0 = builder.block_params(block)[0];
    let arg1 = builder.block_params(block)[1];

    // let result = builder.ins().sadd(arg0, const_val);
    let _ = builder.ins().iconcat(arg0, arg1);
    builder.ins().call(fnref, &[arg0]);

    builder.ins().return_(&[arg1]);
    builder.seal_all_blocks();
    builder.finalize();
  }

  println!("{func:?}");

  let mut ctx = Context::for_function(func);

  let code = ctx
    .compile(isa.as_ref(), &mut Default::default())
    .expect("Compilation Failed");

  let machinecode = code.code_buffer();
  let relocs = code.buffer.relocs();

  println!("{machinecode:?}");
  println!("\nRELOCS:\n{relocs:?}");

  jit(
    machinecode,
    relocs
      .iter()
      .map(|x| Relocation {
        addend: x.addend,
        kind: match x.kind {
          Reloc::Abs8 => RelocKind::Abs8,
          Reloc::Arm64Call => RelocKind::Arm64Call,
          Reloc::X86CallPCRel4 => RelocKind::X86CallPCRel4,
          Reloc::X86PCRel4 => RelocKind::X86PCRel4,
          _ => unreachable!(),
        },
        offset: x.offset,
        symbol_addr: myfn as *const () as usize as _,
      })
      .collect::<Vec<_>>(),
  )
}

fn jit(data: &[u8], reloc: Vec<Relocation>) {
  unsafe {
    println!("What's linked: {}", myfn as *const () as usize);

    let code = MemoryExecutable::new(&data, &reloc).unwrap();

    let e: extern "C" fn(i32, i32) -> i32 = transmute(code.entry_ptr());

    println!("{}", e(10, 20));
  }
}

extern "C" fn myfn(a: i64) {
  println!("Hi, {a}");
}
