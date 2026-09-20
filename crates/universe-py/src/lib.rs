use pyo3::prelude::*;
use serde::de::DeserializeOwned;

pyo3::create_exception!(universe_core, UniverseError, pyo3::exceptions::PyException);

fn err(e: universe::Error) -> PyErr {
    use universe::Error as E;
    let (kind, msg) = match e {
        E::NotFound(m) => ("NotFound", m),
        E::Ambiguous(m) => ("Ambiguous", m),
        E::Busy(m) => ("Busy", m),
        E::Invalid(m) => ("Invalid", m),
        E::Unavailable(m) => ("Unavailable", m),
        E::Io(m) => ("Io", m),
    };
    UniverseError::new_err((kind, msg))
}

fn py_of<T: serde::Serialize + ?Sized>(py: Python<'_>, v: &T) -> PyResult<Py<PyAny>> {
    Ok(pythonize::pythonize(py, v)?.unbind())
}

/// A Python dict/list into the core's type; a shape the type refuses is the core's `Invalid`, as from the CLI.
fn typed<T: DeserializeOwned>(obj: &Bound<'_, PyAny>) -> PyResult<T> {
    let v: serde_json::Value = pythonize::depythonize(obj)?;
    serde_json::from_value(v).map_err(|e| err(e.into()))
}

/// The core, opened once per process; every call releases the GIL and blocks on the runtime.
#[pyclass(frozen)]
struct Core {
    core: universe::core::Core,
    rt: tokio::runtime::Runtime,
}

impl Core {
    fn run<'a, T: Send, F: std::future::Future<Output = universe::Result<T>> + 'a>(&'a self, py: Python<'_>, f: impl FnOnce(&'a universe::core::Core) -> F + Send) -> PyResult<T> {
        py.detach(|| self.rt.block_on(f(&self.core))).map_err(err)
    }

    fn run_infallible<'a, T: Send, F: std::future::Future<Output = T> + 'a>(&'a self, py: Python<'_>, f: impl FnOnce(&'a universe::core::Core) -> F + Send) -> T {
        py.detach(|| self.rt.block_on(f(&self.core)))
    }

    fn value<'a, T: Send + serde::Serialize, F: std::future::Future<Output = universe::Result<T>> + 'a>(&'a self, py: Python<'_>, f: impl FnOnce(&'a universe::core::Core) -> F + Send) -> PyResult<Py<PyAny>> {
        py_of(py, &self.run(py, f)?)
    }

    fn value_infallible<'a, T: Send + serde::Serialize, F: std::future::Future<Output = T> + 'a>(&'a self, py: Python<'_>, f: impl FnOnce(&'a universe::core::Core) -> F + Send) -> PyResult<Py<PyAny>> {
        py_of(py, &self.run_infallible(py, f))
    }
}

type Progress<'a> = Box<dyn FnMut(u64, u64, &str) + 'a>;

/// A Python `progress(done, total, message)` callable, driven from the detached runtime thread.
fn progress_of(cb: &Option<Py<PyAny>>) -> Option<Progress<'_>> {
    cb.as_ref().map(|p| {
        Box::new(move |done: u64, total: u64, message: &str| {
            Python::attach(|py| {
                if let Err(e) = p.call1(py, (done, total, message)) {
                    e.print(py);
                }
            })
        }) as Progress<'_>
    })
}

#[pymethods]
impl Core {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| err(e.into()))?;
        let core = py.detach(|| rt.block_on(universe::core::Core::open())).map_err(err)?;
        Ok(Core { core, rt })
    }

    fn version(&self) -> &'static str {
        universe::BUILD
    }
    fn data_home(&self) -> String {
        universe::paths::data_home().to_string_lossy().to_string()
    }
    fn state_home(&self) -> String {
        universe::paths::state_home().to_string_lossy().to_string()
    }

    fn list(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.list())
    }
    fn get(&self, py: Python<'_>, id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.get(&id).await.map(|r| r.to_json()) })
    }
    fn resolve(&self, py: Python<'_>, query: String) -> Vec<String> {
        self.run_infallible(py, |c| async move { c.resolve(&query).await })
    }
    fn set(&self, py: Python<'_>, id: String, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set(&id, &key, &value).await })
    }
    fn remove(&self, py: Python<'_>, id: String, purge: bool) -> PyResult<()> {
        self.run(py, |c| async move { c.remove(&id, purge).await })
    }
    fn uninstall(&self, py: Python<'_>, id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.uninstall(&id).await })
    }
    fn reload(&self, py: Python<'_>) -> PyResult<()> {
        self.run(py, |c| c.reload_config())
    }
    fn reload_game(&self, py: Python<'_>, id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.reload_game(&id).await })
    }
    fn import_lutris(&self, py: Python<'_>, apply: bool) -> PyResult<Py<PyAny>> {
        self.value(py, |c| c.import_lutris(apply))
    }
    /// `{runner, exe, title, platform}`; returns the new id.
    fn add_game(&self, py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<String> {
        let v: serde_json::Value = pythonize::depythonize(spec)?;
        self.run(py, |c| async move { c.add_game(&v).await })
    }

    fn runners(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.runners())
    }
    fn set_runner_setting(&self, py: Python<'_>, runner: String, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_runner_setting(&runner, &key, &value).await })
    }

    fn current(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.current())
    }
    #[pyo3(signature = (id, screen, splash = String::new()))]
    fn launch(&self, py: Python<'_>, id: String, screen: String, splash: String) -> PyResult<String> {
        self.run(py, |c| async move { c.launch(&id, &screen, &splash).await })
    }
    fn stop(&self, py: Python<'_>, session_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.stop(&session_id).await })
    }
    fn adopt_scope(&self, py: Python<'_>) -> PyResult<String> {
        self.run(py, |c| c.adopt_scope())
    }
    fn session_window(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value(py, |c| c.session_window())
    }
    fn wait_session_window(&self, py: Python<'_>, session_id: String, timeout_ms: u64) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.wait_session_window(&session_id, std::time::Duration::from_millis(timeout_ms)).await })
    }
    fn focus_session(&self, py: Python<'_>) -> PyResult<()> {
        self.run(py, |c| c.focus_session())
    }
    fn focus_pid(&self, py: Python<'_>, pid: u32) -> PyResult<()> {
        self.run(py, |c| async move { c.focus_pid(pid).await })
    }
    fn freeze(&self, py: Python<'_>, on: bool) -> PyResult<()> {
        self.run(py, |c| async move { c.freeze(on).await })
    }
    fn nested(&self) -> bool {
        self.core.nest().is_some()
    }
    fn nest_game_shown(&self) -> PyResult<bool> {
        self.core.nest_game_shown().map_err(err)
    }
    fn nest_overlay(&self, window: u32, input: bool, opacity: u32) -> PyResult<()> {
        self.core.nest_overlay(window, input, opacity).map_err(err)
    }
    fn nest_frame(&self, py: Python<'_>) -> PyResult<Option<String>> {
        py.detach(|| self.core.nest_frame()).map_err(err)
    }
    fn set_fps_limit(&self, py: Python<'_>) -> PyResult<String> {
        self.run(py, |c| c.set_fps_limit())
    }
    #[pyo3(signature = (on = None))]
    fn set_mangohud(&self, py: Python<'_>, on: Option<bool>) -> PyResult<bool> {
        self.run(py, |c| async move { c.set_mangohud(on).await })
    }
    #[pyo3(signature = (filter, sharpness = None))]
    fn nest_filter(&self, filter: String, sharpness: Option<u32>) -> PyResult<()> {
        self.core.nest_filter(&filter, sharpness).map_err(err)
    }
    #[pyo3(signature = (change, value = 0))]
    fn volume(&self, py: Python<'_>, change: String, value: u8) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.volume(&change, value).await })
    }
    fn host_gamescope(&self, py: Python<'_>, screen: String) -> Option<Vec<String>> {
        self.run_infallible(py, |c| async move { c.host_gamescope(&screen).await }).map(|(p, a)| std::iter::once(p).chain(a).collect())
    }
    fn screenshot(&self, py: Python<'_>) -> PyResult<String> {
        self.run(py, |c| c.screenshot())
    }
    fn screenshots(&self, py: Python<'_>, id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.screenshots(&id).await })
    }
    fn remove_screenshot(&self, py: Python<'_>, id: String, name: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_screenshot(&id, &name).await })
    }
    fn sessions(&self, py: Python<'_>, id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.sessions(&id).await })
    }

    fn sources(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.sources())
    }
    fn login_url(&self, py: Python<'_>, source: String) -> PyResult<String> {
        self.run(py, |c| async move { c.source_login_url(&source).await })
    }
    fn login(&self, py: Python<'_>, source: String, code: String) -> PyResult<String> {
        self.run(py, |c| async move { c.source_login(&source, &code).await })
    }
    fn library(&self, py: Python<'_>, source: String, refresh: bool) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.source_library(&source, refresh).await })
    }
    fn search(&self, py: Python<'_>, source: String, query: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.source_search(&source, &query).await })
    }
    fn info(&self, py: Python<'_>, source: String, game_id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.source_info(&source, &game_id).await })
    }
    #[pyo3(signature = (source, game_id, progress=None))]
    fn install(&self, py: Python<'_>, source: String, game_id: String, progress: Option<Py<PyAny>>) -> PyResult<String> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.source_install(&source, &game_id, p.as_deref_mut()).await
        })
    }
    fn cancel(&self, source: String, game_id: String) -> bool {
        self.core.source_cancel(&source, &game_id)
    }
    #[pyo3(signature = (source, game_id, progress=None))]
    fn update(&self, py: Python<'_>, source: String, game_id: String, progress: Option<Py<PyAny>>) -> PyResult<usize> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.source_update(&source, &game_id, p.as_deref_mut()).await
        })
    }
    fn updates(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value(py, |c| c.source_updates())
    }
    #[pyo3(signature = (source, progress=None))]
    fn scan(&self, py: Python<'_>, source: String, progress: Option<Py<PyAny>>) -> PyResult<usize> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.source_scan(&source, p.as_deref_mut()).await
        })
    }

    #[pyo3(signature = (id, force, progress=None))]
    fn media_refresh(&self, py: Python<'_>, id: String, force: bool, progress: Option<Py<PyAny>>) -> PyResult<(usize, usize)> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.media_refresh(&id, force, p.as_deref_mut()).await
        })
    }
    fn media_cancel(&self) {
        self.core.media_cancel();
    }
    fn media_set_slot(&self, py: Python<'_>, id: String, slot: String, path: String) -> PyResult<String> {
        self.run(py, |c| async move { c.media_set_slot(&id, &slot, &path).await })
    }
    fn media_set_url(&self, py: Python<'_>, id: String, slot: String, url: String) -> PyResult<String> {
        self.run(py, |c| async move { c.media_set_url(&id, &slot, &url).await })
    }
    fn media_unset(&self, py: Python<'_>, id: String, slot: String) -> PyResult<bool> {
        self.run(py, |c| async move { c.media_unset(&id, &slot).await })
    }
    #[pyo3(signature = (id, slot, page=0))]
    fn media_candidates(&self, py: Python<'_>, id: String, slot: String, page: u32) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.media_candidates(&id, &slot, page).await })
    }
    fn media_search(&self, py: Python<'_>, id: String, query: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.media_search(&id, &query).await })
    }
    fn media_status(&self, py: Python<'_>, id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.media_status(&id).await })
    }
    fn media_pin(&self, py: Python<'_>, id: String, provider: String, provider_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.media_pin(&id, &provider, &provider_id).await })
    }

    fn file_recording(&self, py: Python<'_>, session_id: String, path: String) -> PyResult<String> {
        self.run(py, |c| async move { c.file_recording(&session_id, &path, None).await })
    }
    fn remove_recording(&self, py: Python<'_>, id: String, session_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_recording(&id, &session_id).await })
    }
    fn remove_journal_entry(&self, py: Python<'_>, id: String, session_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_journal_entry(&id, &session_id).await })
    }
    fn journal(&self, py: Python<'_>, id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.journal(&id).await })
    }
    fn render_journal(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.render_journal(&id).await })
    }
    fn pending_journals(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.pending_journals())
    }
    fn add_entry(&self, py: Python<'_>, session_id: String, entry: &Bound<'_, PyAny>) -> PyResult<()> {
        let entry: universe::journal::Entry = typed(entry)?;
        self.run(py, |c| async move { c.add_entry(&session_id, entry).await })
    }

    fn modules(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.modules())
    }
    fn enable_module(&self, py: Python<'_>, id: String, enabled: bool) -> PyResult<()> {
        self.run(py, |c| async move { c.enable_module(&id, enabled).await })
    }
    fn module_settings(&self, py: Python<'_>, module: String, game_id: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.module_settings(&module, &game_id).await })
    }
    fn module_setting_choices(&self, py: Python<'_>, module: String, key: String) -> PyResult<Vec<String>> {
        self.run(py, |c| async move { c.module_setting_choices(&module, &key).await })
    }
    fn set_module_setting(&self, py: Python<'_>, module: String, game_id: String, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_module_setting(&module, &game_id, &key, &value).await })
    }
    fn enable_source(&self, py: Python<'_>, id: String, enabled: bool) -> PyResult<()> {
        self.run(py, |c| async move { c.enable_source(&id, enabled).await })
    }
    fn source_settings(&self, py: Python<'_>, source: String) -> PyResult<Py<PyAny>> {
        self.value(py, |c| async move { c.source_settings(&source).await })
    }
    fn source_setting_choices(&self, py: Python<'_>, source: String, key: String) -> PyResult<Vec<String>> {
        self.run(py, |c| async move { c.source_setting_choices(&source, &key).await })
    }
    fn set_source_setting(&self, py: Python<'_>, source: String, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_source_setting(&source, &key, &value).await })
    }
    fn settings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.settings())
    }
    fn set_setting(&self, py: Python<'_>, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_setting(&key, &value).await })
    }
    fn screen_mode(&self, py: Python<'_>, screen: String) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| async move { c.screen_mode(&screen).await })
    }
    /// `screen`: a `screen_mode()` dict, or `None` for the screenless choices.
    fn launch_keys(&self, py: Python<'_>, scope: String, screen: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyAny>> {
        let screen: Option<universe::gamescope::Mode> = screen.map(typed).transpose()?;
        py_of(py, &self.core.launch_keys(&scope, screen).map_err(err)?)
    }
    fn gpu(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.gpu())
    }
    fn doctor(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.doctor())
    }
    fn discover(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.discover())
    }

    fn controller_state(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.controller_state())
    }
    fn controller_pads(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.value_infallible(py, |c| c.controller_pads())
    }
    fn set_controller_macro(&self, py: Python<'_>, macro_: &Bound<'_, PyAny>) -> PyResult<()> {
        let m: universe::controller::Macro = typed(macro_)?;
        self.run(py, |c| async move { c.set_controller_macro(m).await })
    }
    fn remove_controller_macro(&self, py: Python<'_>, family: String, button: String, trigger: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_controller_macro(&family, &button, &trigger).await })
    }
    /// `[]` leaves the slot unbound, `None` restores the seeds.
    fn set_controller_button(&self, py: Python<'_>, family: String, slot: String, codes: Option<Vec<String>>) -> PyResult<()> {
        self.run(py, |c| async move { c.set_controller_button(&family, &slot, codes).await })
    }
}

#[pymodule]
fn universe_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    universe::init_tracing();
    m.add_class::<Core>()?;
    m.add("UniverseError", m.py().get_type::<UniverseError>())?;
    Ok(())
}
