pub mod binary;
mod gc;
pub mod lat;
pub mod pure_values;
pub mod runtime;
mod util;

#[cfg(test)]
mod tests {
    use crate::{
        binary::{instructions::StackIndex, modules::ImportSource},
        pure_values::Integer,
        runtime::Runtime,
    };

    #[test]
    fn simple_complete_test() -> anyhow::Result<()> {
        let module_set = super::lat::from_str(
            r#"
                (module-set
                    ("test"
                        (export test_func)
                        (const test_func
                            (fn 
                                (add)
                                (return 1)))))
            "#,
        )?;

        let runtime = Runtime::new();
        runtime.load_module_set(&module_set)?;

        let top_level = runtime.make_top_level();
        {
            let mut stack = top_level.stack();
            stack.push_int(1);
            stack.push_int(2);
            stack.push_import(&ImportSource::new(["test"], "test_func"))?;
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
        let runtime = Runtime::new();
        let top_level = runtime.make_top_level();
        {
            let mut stack = top_level.stack();
            stack.push_int(1);
            stack.push_int(2);
            stack.push_native_function(|mut ctxt| {
                {
                    let mut stack = ctxt.stack();
                    let i1 = stack.get_int(StackIndex::FromTop(0))?;
                    let i2 = stack.get_int(StackIndex::FromTop(1))?;
                    stack.pop_n(2)?;
                    stack.push_int(i1.add_owned(i2));
                }
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

    #[test]
    // #[ignore = "Not all opcodes implemented"]
    fn simple_recursive_function_test() -> anyhow::Result<()> {
        let module_set = super::lat::from_str(
            r#"
                (module-set
                    ("test"
                        (const fib_inner
                            (fn
                                ; Test if iterations is 0
                                (push_copy bot 0)
                                (push 0)
                                (cmp ref_eq)
                                (branch_if #:end)

                                (push fib_inner)

                                ; Subtract one from iterations
                                (push_copy bot 0)
                                (push -1)
                                (add)

                                (push_copy bot 2)
                                (push_copy bot 2)
                                (push_copy bot 1)
                                (add)
                                (tail_call 3)
                                #:end
                                (push_copy bot 2)
                                (return 1)
                                ))
                        (const fib
                            (fn 
                                (push fib_inner)
                                (push_copy bot 0)
                                (push 0)
                                (push 1)
                                (call 3 1)
                                (return 1)))
                        (export fib)))
            "#,
        )?;
        let runtime = Runtime::new();
        runtime.load_module_set(&module_set)?;

        let top_level = runtime.make_top_level();
        {
            let mut stack = top_level.stack();
            stack.push_int(9);
            stack.push_import(&ImportSource::new(["test"], "fib"))?;
        }
        top_level.call_function(1)?;
        assert_eq!(
            Integer::from(55),
            top_level.stack().get_int(StackIndex::FromTop(0))?
        );
        Ok(())
    }
}
