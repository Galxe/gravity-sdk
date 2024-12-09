pub struct Call<Input, Output> {
    f: Box<dyn Fn(Input) -> Result<Output, ()> + Send + Sync>,
}

impl <Input, Output> Call<Input, Output> {
    pub fn new<F>(f: F) -> Self
    where F: Fn(Input) -> Result<Output, ()> + 'static + Send + Sync
    {
        Self { f: Box::new(f) }
    }

    pub fn call(&self, input: Input) -> Result<Output, ()> {
        (self.f)(input)
    }


}