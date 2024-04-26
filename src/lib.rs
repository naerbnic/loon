mod refs;
pub mod runtime;
pub mod binary;
pub mod pure_values;

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "symbol not found")]
    fn basic_initialization() {
        crate::runtime::RuntimeBuilder::new()
            .build_with_main("main")
            .unwrap();
    }
}
