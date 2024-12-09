pub struct Call<Input, Output> {
    f: Box<dyn Fn(Input) -> Result<Output, ()>>,
}

impl <Input, Output> Call<Input, Output> {
    pub fn new<F>(f: F) -> Self
    where F: Fn(Input) -> Result<Output, ()> + 'static
    {
        Self { f: Box::new(f) }
    }

    pub fn call(&self, input: Input) -> Result<Output, ()> {
        (self.f)(input)
    }


}