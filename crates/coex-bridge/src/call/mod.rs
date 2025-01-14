use std::marker::PhantomData;

use async_trait::async_trait;
use futures::future::BoxFuture;

enum FuncEnum<Func, AsyncFunc, Input, Output>
where
    Func: Fn(Input) -> Result<Output, ()> + Send + Sync,
    AsyncFunc: Fn(Input) -> BoxFuture<'static, Result<Output, ()>> + Send + Sync,
{
    Func(Func),
    AysncFunc(AsyncFunc),
    PhantomData(PhantomData<(Input, Output)>),
}

pub struct Call<Input, Output> {
    f: Box<dyn Fn(Input) -> Result<Output, ()> + Send + Sync>,
}

impl<Input, Output> Call<Input, Output> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Input) -> Result<Output, ()> + 'static + Send + Sync,
    {
        Self { f: Box::new(f) }
    }

    pub fn call(&self, input: Input) -> Result<Output, ()> {
        (self.f)(input)
    }
}

#[async_trait]
pub trait AsyncCallImplTrait: Send + Sync {
    type Input;
    type Output;
    async fn call(&self, input: Self::Input) -> Result<Self::Output, ()>;
}

pub struct AsyncCall<Input, Output> {
    f: Box<dyn AsyncCallImplTrait<Input = Input, Output = Output>>,
}

impl<Input, Output> AsyncCall<Input, Output> {
    pub fn new(f: Box<dyn AsyncCallImplTrait<Input = Input, Output = Output>>) -> Self {
        Self { f }
    }

    pub async fn call(&self, input: Input) -> Result<Output, ()> {
        self.f.call(input).await
    }
}
