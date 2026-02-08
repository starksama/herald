use std::future::Future;
use std::marker::PhantomData;

pub mod layers {
    #[derive(Clone, Copy)]
    pub struct RetryLayer<F> {
        _policy: F,
    }

    impl<F> RetryLayer<F> {
        pub fn new(policy: F) -> Self {
            Self { _policy: policy }
        }
    }
}

pub mod postgres {
    use anyhow::Result;
    use std::marker::PhantomData;

    #[derive(Clone)]
    pub struct PostgresStorage<T> {
        _marker: PhantomData<T>,
    }

    impl<T> PostgresStorage<T> {
        pub async fn new(_database_url: &str) -> Result<Self> {
            Ok(Self {
                _marker: PhantomData,
            })
        }

        pub async fn push(&self, _queue: &str, _job: T) -> Result<()> {
            Ok(())
        }
    }
}

pub mod prelude {
    pub use crate::monitor::Monitor;
    pub use crate::worker::WorkerBuilder;
}

mod worker {
    use super::*;

    #[derive(Clone)]
    pub struct Worker<T> {
        _queue: String,
        _marker: PhantomData<T>,
    }

    pub struct WorkerBuilder<T> {
        queue: String,
        _marker: PhantomData<T>,
    }

    impl<T> WorkerBuilder<T> {
        pub fn new(queue: &str) -> Self {
            Self {
                queue: queue.to_string(),
                _marker: PhantomData,
            }
        }

        pub fn layer<L>(self, _layer: L) -> Self {
            self
        }

        pub fn build_fn<F, Fut>(self, _handler: F) -> Worker<T>
        where
            F: Fn(T) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
        {
            Worker {
                _queue: self.queue,
                _marker: PhantomData,
            }
        }
    }
}

mod monitor {
    use super::worker::Worker;

    pub struct Monitor<T> {
        _workers: Vec<Worker<T>>,
    }

    impl<T> Default for Monitor<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> Monitor<T> {
        pub fn new() -> Self {
            Self { _workers: vec![] }
        }

        pub fn register(mut self, worker: Worker<T>) -> Self {
            self._workers.push(worker);
            self
        }

        pub async fn run(self) -> anyhow::Result<()> {
            Ok(())
        }
    }
}
