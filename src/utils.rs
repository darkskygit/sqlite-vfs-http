use super::*;
use reqwest::Client;
use std::{future::Future, thread::spawn};
use tokio::runtime::Runtime;

#[derive(Clone)]
pub struct AtomicRuntime {
    client: Client,
    rt: Arc<RwLock<Option<Runtime>>>,
}

impl Default for AtomicRuntime {
    fn default() -> Self {
        Self {
            client: Client::new(),
            rt: Arc::new(RwLock::new(Runtime::new().ok())),
        }
    }
}

impl AtomicRuntime {
    pub fn block_on<C, F>(&self, cb: C) -> Option<F::Output>
    where
        C: FnOnce(Client) -> F + Send + 'static,
        F: Future<Output: Send + Sync> + 'static,
    {
        let client = self.client.clone();
        let rt = self.rt.clone();
        if let Ok(Some(ret)) = spawn(move || {
            let rt = rt.read().unwrap();
            rt.as_ref().map(|rt| rt.block_on(cb(client)))
        })
        .join()
        {
            Some(ret)
        } else {
            None
        }
    }

    pub fn drop(&self) {
        if let Some(rt) = self.rt.write().unwrap().take() {
            rt.shutdown_background();
        }
    }
}
