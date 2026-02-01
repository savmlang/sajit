use cranelift::{
  codegen::{
    Context,
    ir::{Function, UserExternalName, UserFuncName},
  },
  prelude::{isa::lookup, settings::Flags, types::I64, *},
};
use target_lexicon::Triple;

fn main() {
  let builder = settings::builder();

  let isa = lookup(Triple::host())
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
}
