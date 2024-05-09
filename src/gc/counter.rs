use std::cell::Cell;

pub struct Counter(Cell<usize>);

impl Counter {
    pub fn new() -> Self {
        Counter(Cell::new(0))
    }

    pub fn increment(&self) {
        let value = self.0.get();
        self.0.set(value.checked_add(1).expect("Counter overflow"));
    }

    pub fn decrement(&self) {
        let value = self.0.get();
        self.0.set(value.checked_sub(1).expect("Counter underflow"));
    }

    pub fn is_nonzero(&self) -> bool {
        self.0.get() != 0
    }

    pub fn is_zero(&self) -> bool {
        self.0.get() == 0
    }
}
