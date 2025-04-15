use proptest::prelude::Strategy;

#[derive(Default)]
pub struct ValueGenerator {
}

#[derive(Clone, Debug)]
pub struct TestRng {
    rng: TestRngImpl,
}

#[derive(Clone, Debug)]
enum TestRngImpl {
}

impl ValueGenerator {
    /// Creates a new value generator with the default RNG.
    pub fn new() -> Self {
        Default::default()
    }

    /// Creates a new value generator with provided RNG
    pub fn new_with_rng(rng: TestRng) -> Self {
        todo!()
    }

    /// Creates a new value generator with a deterministic RNG.
    ///
    /// This generator has a hardcoded seed, so its results are predictable across test runs.
    /// However, a new proptest version may change the seed.
    pub fn deterministic() -> Self {
        todo!()
    }

    /// Generates a single value for this strategy.
    ///
    /// Panics if generating the new value fails. The only situation in which this can happen is if
    /// generating the value causes too many internal rejects.
    pub fn generate<S: Strategy>(&mut self, strategy: S) -> S::Value {
        todo!()
    }
}
