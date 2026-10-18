use std::future::Future;
use std::sync::{Arc, OnceLock};

use tokio::runtime::{Handle, Runtime};
use universe::core::Core;

static RUNTIME: OnceLock<Handle> = OnceLock::new();
static CORE: OnceLock<Arc<Core>> = OnceLock::new();

/// The core's runtime, owned by `main` so it can stop it before the process exits.
pub fn start() -> Runtime {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).thread_name("universe-core").enable_all().build().expect("the core's runtime");
    let _ = RUNTIME.set(runtime.handle().clone());
    runtime
}

pub fn runtime() -> &'static Handle {
    RUNTIME.get().expect("the core's runtime is started")
}

/// Set once, when `open` succeeds; nothing asks for it before the window has a library.
pub fn core() -> Arc<Core> {
    CORE.get().expect("the core is open").clone()
}

/// Runs `work` on the core's runtime and hands its value to whoever awaits it, the GTK main loop included.
pub async fn run<T: Send + 'static>(work: impl Future<Output = T> + Send + 'static) -> T {
    let (tx, rx) = tokio::sync::oneshot::channel();
    runtime().spawn(async move {
        let _ = tx.send(work.await);
    });
    rx.await.expect("a core task panicked")
}

/// `f` with the open core, on its runtime.
pub async fn call<T, F, Fut>(f: F) -> universe::Result<T>
where
    T: Send + 'static,
    F: FnOnce(Arc<Core>) -> Fut + Send + 'static,
    Fut: Future<Output = universe::Result<T>> + Send + 'static,
{
    let core = core();
    run(async move { f(core).await }).await
}

/// `call` for a core future the compiler cannot prove `Send` (`launch`, a source's verbs): polled to the end on a blocking thread
/// of the runtime.
pub async fn pinned<T, F, Fut>(f: F) -> T
where
    T: Send + 'static,
    F: FnOnce(Arc<Core>) -> Fut + Send + 'static,
    Fut: Future<Output = T> + 'static,
{
    let core = core();
    let handle = runtime().clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    runtime().spawn_blocking(move || {
        let _ = tx.send(handle.block_on(f(core)));
    });
    rx.await.expect("a core task panicked")
}

pub async fn open() -> universe::Result<()> {
    let core = run(Core::open()).await?;
    let _ = CORE.set(Arc::new(core));
    Ok(())
}
