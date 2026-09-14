use pyo3::prelude::*;

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
}

/// A Python `progress(done, total, message)` callable, driven from the detached runtime thread.
fn progress_of(cb: &Option<Py<PyAny>>) -> Option<Box<dyn FnMut(u64, u64, &str) + '_>> {
    cb.as_ref().map(|p| {
        Box::new(move |done: u64, total: u64, message: &str| {
            Python::attach(|py| {
                if let Err(e) = p.call1(py, (done, total, message)) {
                    e.print(py);
                }
            })
        }) as Box<dyn FnMut(u64, u64, &str)>
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

    // -- library --
    fn list_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.list_json())
    }
    fn get_json(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.get(&id).await.map(|r| r.to_json().to_string()) })
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
    fn import_lutris(&self, py: Python<'_>, apply: bool) -> PyResult<String> {
        self.run(py, |c| c.import_lutris(apply))
    }
    fn add_game(&self, py: Python<'_>, json: String) -> PyResult<String> {
        self.run(py, |c| async move { c.add_game(&json).await })
    }

    // -- runners --
    fn runners_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.runners_json())
    }
    fn set_runner_setting(&self, py: Python<'_>, runner: String, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_runner_setting(&runner, &key, &value).await })
    }

    // -- session --
    fn current_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.current_json())
    }
    fn launch(&self, py: Python<'_>, id: String, screen: String) -> PyResult<String> {
        self.run(py, |c| async move { c.launch(&id, &screen).await })
    }
    fn stop(&self, py: Python<'_>, session_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.stop(&session_id).await })
    }
    fn adopt_scope(&self, py: Python<'_>) -> PyResult<String> {
        self.run(py, |c| c.adopt_scope())
    }
    fn screenshot(&self, py: Python<'_>) -> PyResult<String> {
        self.run(py, |c| c.screenshot())
    }
    fn sessions_json(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.sessions_json(&id).await })
    }

    // -- sources --
    fn sources_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.sources_json())
    }
    fn login_url(&self, py: Python<'_>, source: String) -> PyResult<String> {
        self.run(py, |c| async move { c.source_login_url(&source).await })
    }
    fn login(&self, py: Python<'_>, source: String, code: String) -> PyResult<String> {
        self.run(py, |c| async move { c.source_login(&source, &code).await })
    }
    fn library_json(&self, py: Python<'_>, source: String, refresh: bool) -> PyResult<String> {
        self.run(py, |c| async move { c.source_library(&source, refresh).await })
    }
    fn search_json(&self, py: Python<'_>, source: String, query: String) -> PyResult<String> {
        self.run(py, |c| async move { c.source_search(&source, &query).await })
    }
    fn info_json(&self, py: Python<'_>, source: String, game_id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.source_info(&source, &game_id).await })
    }
    #[pyo3(signature = (source, game_id, progress=None))]
    fn install(&self, py: Python<'_>, source: String, game_id: String, progress: Option<Py<PyAny>>) -> PyResult<String> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.source_install(&source, &game_id, p.as_deref_mut()).await
        })
    }
    #[pyo3(signature = (source, game_id, progress=None))]
    fn update(&self, py: Python<'_>, source: String, game_id: String, progress: Option<Py<PyAny>>) -> PyResult<usize> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.source_update(&source, &game_id, p.as_deref_mut()).await
        })
    }
    fn updates_json(&self, py: Python<'_>) -> PyResult<String> {
        self.run(py, |c| c.source_updates())
    }
    #[pyo3(signature = (source, progress=None))]
    fn scan(&self, py: Python<'_>, source: String, progress: Option<Py<PyAny>>) -> PyResult<usize> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.source_scan(&source, p.as_deref_mut()).await
        })
    }

    // -- media --
    #[pyo3(signature = (id, force, progress=None))]
    fn media_refresh(&self, py: Python<'_>, id: String, force: bool, progress: Option<Py<PyAny>>) -> PyResult<(usize, usize)> {
        self.run(py, |c| async move {
            let mut p = progress_of(&progress);
            c.media_refresh(&id, force, p.as_deref_mut()).await
        })
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
    fn media_candidates_json(&self, py: Python<'_>, id: String, slot: String, page: u32) -> PyResult<String> {
        self.run(py, |c| async move { c.media_candidates(&id, &slot, page).await })
    }
    fn media_search_json(&self, py: Python<'_>, id: String, query: String) -> PyResult<String> {
        self.run(py, |c| async move { c.media_search(&id, &query).await })
    }
    fn media_status_json(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.media_status(&id).await })
    }
    fn media_pin(&self, py: Python<'_>, id: String, provider: String, provider_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.media_pin(&id, &provider, &provider_id).await })
    }

    // -- recordings & journal --
    fn recordings_json(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.recordings_json(&id).await })
    }
    fn file_recording(&self, py: Python<'_>, session_id: String, path: String) -> PyResult<String> {
        self.run(py, |c| async move { c.file_recording(&session_id, &path).await })
    }
    fn remove_recording(&self, py: Python<'_>, id: String, session_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_recording(&id, &session_id).await })
    }
    fn remove_journal_entry(&self, py: Python<'_>, id: String, session_id: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_journal_entry(&id, &session_id).await })
    }
    fn journal_json(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.journal_json(&id).await })
    }
    fn render_journal(&self, py: Python<'_>, id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.render_journal(&id).await })
    }
    fn pending_journals_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.pending_journals_json())
    }
    fn add_entry(&self, py: Python<'_>, session_id: String, json: String) -> PyResult<()> {
        self.run(py, |c| async move { c.add_entry(&session_id, &json).await })
    }

    // -- modules & settings --
    fn modules_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.modules_json())
    }
    fn enable_module(&self, py: Python<'_>, id: String, enabled: bool) -> PyResult<()> {
        self.run(py, |c| async move { c.enable_module(&id, enabled).await })
    }
    fn module_settings_json(&self, py: Python<'_>, module: String, game_id: String) -> PyResult<String> {
        self.run(py, |c| async move { c.module_settings_json(&module, &game_id).await })
    }
    fn module_setting_choices_json(&self, py: Python<'_>, module: String, key: String) -> PyResult<String> {
        self.run(py, |c| async move { c.module_setting_choices(&module, &key).await })
    }
    fn set_module_setting(&self, py: Python<'_>, module: String, game_id: String, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_module_setting(&module, &game_id, &key, &value).await })
    }
    fn settings_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.settings_json())
    }
    fn set_setting(&self, py: Python<'_>, key: String, value: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_setting(&key, &value).await })
    }
    fn doctor_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.doctor_json())
    }

    // -- controller --
    fn controller_state_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.controller_state_json())
    }
    fn controller_pads_json(&self, py: Python<'_>) -> String {
        self.run_infallible(py, |c| c.controller_pads_json())
    }
    fn set_controller_macro(&self, py: Python<'_>, json: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_controller_macro(&json).await })
    }
    fn remove_controller_macro(&self, py: Python<'_>, family: String, button: String, trigger: String) -> PyResult<()> {
        self.run(py, |c| async move { c.remove_controller_macro(&family, &button, &trigger).await })
    }
    fn set_controller_button(&self, py: Python<'_>, family: String, slot: String, codes_json: String) -> PyResult<()> {
        self.run(py, |c| async move { c.set_controller_button(&family, &slot, &codes_json).await })
    }
}

#[pyfunction]
fn version() -> &'static str {
    universe::VERSION
}

#[pyfunction]
fn data_home() -> String {
    universe::paths::data_home().to_string_lossy().to_string()
}

#[pyfunction]
fn state_home() -> String {
    universe::paths::state_home().to_string_lossy().to_string()
}

#[pymodule]
fn universe_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Core>()?;
    m.add("UniverseError", m.py().get_type::<UniverseError>())?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(data_home, m)?)?;
    m.add_function(wrap_pyfunction!(state_home, m)?)?;
    Ok(())
}
