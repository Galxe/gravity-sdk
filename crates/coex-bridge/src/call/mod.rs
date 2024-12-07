pub struct Call<Input, Output> {
    f: Box<dyn Fn(Input) -> Result<Output, ()>>,
}

impl <Input, Output> Call<Input, Output> {
    pub fn new(f: Box<dyn Fn(Input) -> Result<Output, ()>>) -> Self {
        Self { f }
    }

    pub fn call(&self, input: Input) -> Result<Output, ()> {
        (self.f)(input)
    }
}