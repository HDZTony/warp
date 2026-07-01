use std::future::Future;
use std::sync::Arc;

use tokio::runtime::Runtime;
use wormhole_desktop_core::desktop_runtime_spawn;
use wormhole_desktop_core::DesktopRuntime;

#[derive(Clone)]
pub struct CoreHandle {
    runtime: Arc<DesktopRuntime>,
    tokio: Arc<Runtime>,
}

impl CoreHandle {
    pub fn new(runtime: DesktopRuntime, tokio: Runtime) -> Self {
        let tokio = Arc::new(tokio);
        desktop_runtime_spawn::register_desktop_runtime(Arc::clone(&tokio));
        Self {
            runtime: Arc::new(runtime),
            tokio,
        }
    }

    pub fn runtime(&self) -> Arc<DesktopRuntime> {
        Arc::clone(&self.runtime)
    }

    pub fn data_dir(&self) -> std::path::PathBuf {
        self.runtime.data_dir().to_path_buf()
    }

    pub fn block_on<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T>,
    {
        self.tokio.block_on(fut)
    }

    pub fn app_state(&self) -> &wormhole_desktop_core::state::AppState {
        &self.runtime.state
    }
}
