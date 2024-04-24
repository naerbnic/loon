use crate::runtime::value::Value;

pub struct TailCall {
    function: Value,
    args: Vec<Value>,
}

pub struct CallWithContinuation {
    function: Value,
    args: Vec<Value>,
    continuation: Value,
}

enum NativeFunctionResult {
    ReturnValue(Vec<Value>),
    TailCall(TailCall),
    CallWithContinuation(CallWithContinuation),
}

struct NativeFunctionContext();

impl NativeFunctionContext {
    pub fn call(&self, function: Value, args: &[Value]) -> Vec<Value> {
        todo!()
    }

    pub fn return_with(self, values: Vec<Value>) -> NativeFunctionResult {
        NativeFunctionResult::ReturnValue(values)
    }

    pub fn tail_call(self, function: Value, args: Vec<Value>) -> NativeFunctionResult {
        NativeFunctionResult::TailCall(TailCall { function, args })
    }

    pub fn call_with_continuation(
        self,
        function: Value,
        args: Vec<Value>,
        continuation: Value,
    ) -> NativeFunctionResult {
        NativeFunctionResult::CallWithContinuation(CallWithContinuation {
            function,
            args,
            continuation,
        })
    }
}

pub trait NativeFunction {
    fn call(&self, ctxt: NativeFunctionContext, args: &[Value]) -> NativeFunctionResult;
}
