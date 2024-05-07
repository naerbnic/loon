pub mod binary;
mod gc;
pub mod pure_values;
pub mod runtime;
mod util;

#[cfg(test)]
mod tests {
    use crate::{
        binary::{
            instructions::StackIndex,
            modules::{ImportSource, ModuleId, ModuleMemberId},
            ModuleBuilder,
        },
        pure_values::Integer,
        runtime::{global_env::GlobalEnv, TopLevelRuntime},
    };

    #[test]
    fn simple_complete_test() -> anyhow::Result<()> {
        let module_id = ModuleId::new(["test"]);
        let member_id = ModuleMemberId::new("test_func");
        let module_builder = ModuleBuilder::with_num_globals(0);
        let (test_func, mut test_func_builder) = module_builder.new_function();
        test_func.export(member_id.clone())?;
        test_func_builder.add().return_(1);
        test_func_builder.build()?;
        let module = module_builder.into_const_module()?;

        let global_env = GlobalEnv::new();
        global_env.load_module(module_id.clone(), &module)?;

        let mut top_level = TopLevelRuntime::new(global_env);
        {
            let mut stack = top_level.stack();
            stack.push_int(1);
            stack.push_int(2);
            stack.push_import(&ImportSource::new(module_id.clone(), member_id.clone()))?;
        }
        let num_args = top_level.call_function(2)?;
        assert_eq!(num_args, 1);
        assert_eq!(
            Integer::from(3),
            top_level.stack().get_int(StackIndex::FromTop(0))?
        );
        Ok(())
    }

    #[test]
    fn simple_native_function_test() -> anyhow::Result<()> {
        let global_env = GlobalEnv::new();
        let mut top_level = TopLevelRuntime::new(global_env);
        {
            let mut stack = top_level.stack();
            stack.push_int(1);
            stack.push_int(2);
            stack.push_native_function(|mut ctxt| {
                let mut stack = ctxt.stack();
                let i1 = stack.get_int(StackIndex::FromTop(0))?;
                let i2 = stack.get_int(StackIndex::FromTop(1))?;
                stack.pop_n(2)?;
                stack.push_int(i1.add_owned(i2));
                Ok(ctxt.return_with(1))
            });
        }
        let num_args = top_level.call_function(2)?;
        assert_eq!(num_args, 1);
        assert_eq!(
            Integer::from(3),
            top_level.stack().get_int(StackIndex::FromTop(0))?
        );
        Ok(())
    }
}
