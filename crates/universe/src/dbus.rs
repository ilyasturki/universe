use std::sync::Arc;

use zbus::object_server::SignalEmitter;
use zbus::{fdo, interface};

use crate::core::{Core, Event};
use crate::{BUS_NAME, OBJECT_PATH};


pub struct Library1(pub Arc<Core>);
pub struct Session1(pub Arc<Core>);
pub struct Sources1(pub Arc<Core>);
pub struct Media1(pub Arc<Core>);
pub struct Recording1(pub Arc<Core>);
pub struct Journal1(pub Arc<Core>);
pub struct Modules1(pub Arc<Core>);
pub struct Settings1(pub Arc<Core>);

#[interface(name = "io.github.ilyasturki.Universe.Library1")]
impl Library1 {
    async fn list(&self) -> String {
        self.0.list_json().await
    }
    async fn get(&self, id: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.get(&id).await?.to_json().to_string())
    }
    async fn resolve(&self, query: String) -> Vec<String> {
        self.0.resolve(&query).await
    }
    async fn set(&self, id: String, key: String, value: String) -> fdo::Result<()> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.set(&id, &key, &value).await?)
    }
    async fn remove(&self, id: String, purge: bool) -> fdo::Result<()> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.remove(&id, purge).await?)
    }
    async fn rescan(&self) -> fdo::Result<()> {
        self.0.reload_config().await?;
        let _ = self.0.source_scan("").await;
        Ok(())
    }
    async fn import_lutris(&self, apply: bool) -> fdo::Result<String> {
        Ok(self.0.import_lutris(apply).await?)
    }
    #[zbus(signal)]
    pub async fn library_changed(emitter: &SignalEmitter<'_>, ids: Vec<String>) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Session1")]
impl Session1 {
    async fn launch(&self, id: String, screen: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.launch(&id, &screen).await?)
    }
    async fn stop(&self, session_id: String) -> fdo::Result<()> {
        Ok(self.0.stop(&session_id).await?)
    }
    async fn screenshot(&self) -> fdo::Result<String> {
        Ok(self.0.screenshot().await?)
    }
    async fn sessions(&self, id: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.sessions_json(&id).await?)
    }
    #[zbus(property)]
    async fn current(&self) -> String {
        self.0.current_json().await
    }
    #[zbus(signal)]
    pub async fn session_started(emitter: &SignalEmitter<'_>, session_id: &str, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    pub async fn session_ended(emitter: &SignalEmitter<'_>, session_id: &str, id: &str, duration_s: u32) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Sources1")]
impl Sources1 {
    async fn list(&self) -> String {
        self.0.sources_json().await
    }
    async fn login_url(&self, source: String) -> fdo::Result<String> {
        Ok(self.0.source_login_url(&source).await?)
    }
    async fn login(&self, source: String, code: String) -> fdo::Result<String> {
        Ok(self.0.source_login(&source, &code).await?)
    }
    async fn library(&self, source: String) -> fdo::Result<String> {
        Ok(self.0.source_library(&source, false).await?)
    }
    async fn refresh_library(&self, source: String) -> fdo::Result<String> {
        Ok(self.0.source_library(&source, true).await?)
    }
    async fn search(&self, source: String, query: String) -> fdo::Result<String> {
        Ok(self.0.source_search(&source, &query).await?)
    }
    async fn info(&self, source: String, game_id: String) -> fdo::Result<String> {
        Ok(self.0.source_info(&source, &game_id).await?)
    }
    async fn install(&self, source: String, game_id: String) -> fdo::Result<String> {
        Ok(self.0.source_install(&source, &game_id).await?)
    }
    async fn update(&self, source: String, game_id: String) -> fdo::Result<String> {
        Ok(self.0.source_update(&source, &game_id).await?)
    }
    async fn updates(&self) -> fdo::Result<String> {
        Ok(self.0.source_updates().await?)
    }
    async fn scan(&self, source: String) -> fdo::Result<String> {
        Ok(self.0.source_scan(&source).await?)
    }
    async fn jobs(&self) -> String {
        self.0.jobs_json().await
    }
    #[zbus(signal)]
    pub async fn progress(emitter: &SignalEmitter<'_>, job_id: &str, done: u64, total: u64, message: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    pub async fn job_finished(emitter: &SignalEmitter<'_>, job_id: &str, ok: bool, message: &str) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Media1")]
impl Media1 {
    async fn refresh(&self, id: String, force: bool) -> fdo::Result<String> {
        Ok(self.0.media_refresh(&id, force).await?)
    }
    async fn set_slot(&self, id: String, slot: String, path: String) -> fdo::Result<()> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.media_set_slot(&id, &slot, &path).await?)
    }
    async fn unset(&self, id: String, slot: String) -> fdo::Result<()> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.media_unset(&id, &slot).await?)
    }
    async fn candidates(&self, id: String, slot: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.media_candidates(&id, &slot).await?)
    }
    async fn pin(&self, id: String, provider: String, provider_id: String) -> fdo::Result<()> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.media_pin(&id, &provider, &provider_id).await?)
    }
    #[zbus(signal)]
    pub async fn media_changed(emitter: &SignalEmitter<'_>, id: &str) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Recording1")]
impl Recording1 {
    async fn file(&self, session_id: String, path: String) -> fdo::Result<String> {
        Ok(self.0.file_recording(&session_id, &path).await?)
    }
    async fn list(&self, id: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.recordings_json(&id).await?)
    }
    #[zbus(signal)]
    pub async fn recording_filed(emitter: &SignalEmitter<'_>, session_id: &str, id: &str, path: &str) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Journal1")]
impl Journal1 {
    async fn add_entry(&self, session_id: String, json: String) -> fdo::Result<()> {
        Ok(self.0.add_entry(&session_id, &json).await?)
    }
    async fn list(&self, id: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.journal_json(&id).await?)
    }
    async fn render(&self, id: String) -> fdo::Result<String> {
        let id = self.0.resolve_one(&id).await?;
        Ok(self.0.render_journal(&id).await?)
    }
    #[zbus(signal)]
    pub async fn entry_written(emitter: &SignalEmitter<'_>, session_id: &str, id: &str) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Modules1")]
impl Modules1 {
    async fn list(&self) -> String {
        self.0.modules_json().await
    }
    async fn enable(&self, id: String, enabled: bool) -> fdo::Result<()> {
        Ok(self.0.enable_module(&id, enabled).await?)
    }
    async fn get_settings(&self, module: String, game_id: String) -> fdo::Result<String> {
        let gid = if game_id.is_empty() { String::new() } else { self.0.resolve_one(&game_id).await? };
        Ok(self.0.module_settings_json(&module, &gid).await?)
    }
    async fn set_setting(&self, module: String, game_id: String, key: String, value: String) -> fdo::Result<()> {
        let gid = if game_id.is_empty() { String::new() } else { self.0.resolve_one(&game_id).await? };
        Ok(self.0.set_module_setting(&module, &gid, &key, &value).await?)
    }
    async fn doctor(&self) -> String {
        self.0.doctor_json().await
    }
    #[zbus(signal)]
    pub async fn modules_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

#[interface(name = "io.github.ilyasturki.Universe.Settings1")]
impl Settings1 {
    async fn get(&self) -> String {
        self.0.settings_json().await
    }
    async fn set(&self, key: String, value: String) -> fdo::Result<()> {
        Ok(self.0.set_setting(&key, &value).await?)
    }
    async fn reload(&self) -> fdo::Result<()> {
        Ok(self.0.reload_config().await?)
    }
    #[zbus(property(emits_changed_signal = "const"))]
    async fn version(&self) -> String {
        crate::VERSION.into()
    }
}

/// Serves every interface at the object path and relays core events as D-Bus signals until the process ends.
pub async fn serve(core: Arc<Core>) -> zbus::Result<zbus::Connection> {
    // Subscribed before the bus name is taken: a job finished during the startup login probe must still signal.
    let mut rx = core.events.subscribe();
    let conn = zbus::connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, Library1(core.clone()))?
        .serve_at(OBJECT_PATH, Session1(core.clone()))?
        .serve_at(OBJECT_PATH, Sources1(core.clone()))?
        .serve_at(OBJECT_PATH, Media1(core.clone()))?
        .serve_at(OBJECT_PATH, Recording1(core.clone()))?
        .serve_at(OBJECT_PATH, Journal1(core.clone()))?
        .serve_at(OBJECT_PATH, Modules1(core.clone()))?
        .serve_at(OBJECT_PATH, Settings1(core.clone()))?
        .build()
        .await?;
    let _ = core.conn.set(conn.clone());
    let c2 = conn.clone();
    tokio::spawn(async move {
        loop {
            let ev = match rx.recv().await {
                Ok(e) => e,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            };
            if let Err(e) = relay(&c2, ev).await {
                tracing::warn!("signal relay: {e}");
            }
        }
    });
    core.load_source_caches().await;
    Ok(conn)
}

async fn relay(conn: &zbus::Connection, ev: Event) -> zbus::Result<()> {
    let os = conn.object_server();
    match ev {
        Event::LibraryChanged(ids) => {
            let r = os.interface::<_, Library1>(OBJECT_PATH).await?;
            Library1::library_changed(r.signal_emitter(), ids).await
        }
        Event::SessionStarted { session, id } => {
            let r = os.interface::<_, Session1>(OBJECT_PATH).await?;
            Session1::session_started(r.signal_emitter(), &session, &id).await
        }
        Event::SessionEnded { session, id, duration_s } => {
            let r = os.interface::<_, Session1>(OBJECT_PATH).await?;
            Session1::session_ended(r.signal_emitter(), &session, &id, duration_s).await
        }
        Event::CurrentChanged(_) => {
            let r = os.interface::<_, Session1>(OBJECT_PATH).await?;
            let iface = r.get().await;
            iface.current_changed(r.signal_emitter()).await
        }
        Event::RecordingFiled { session, id, path } => {
            let r = os.interface::<_, Recording1>(OBJECT_PATH).await?;
            Recording1::recording_filed(r.signal_emitter(), &session, &id, &path).await
        }
        Event::EntryWritten { session, id } => {
            let r = os.interface::<_, Journal1>(OBJECT_PATH).await?;
            Journal1::entry_written(r.signal_emitter(), &session, &id).await
        }
        Event::Progress { job, done, total, message } => {
            let r = os.interface::<_, Sources1>(OBJECT_PATH).await?;
            Sources1::progress(r.signal_emitter(), &job, done, total, &message).await
        }
        Event::JobFinished { job, ok, message } => {
            let r = os.interface::<_, Sources1>(OBJECT_PATH).await?;
            Sources1::job_finished(r.signal_emitter(), &job, ok, &message).await
        }
        Event::MediaChanged(id) => {
            let r = os.interface::<_, Media1>(OBJECT_PATH).await?;
            Media1::media_changed(r.signal_emitter(), &id).await
        }
        Event::ModulesChanged => {
            let r = os.interface::<_, Modules1>(OBJECT_PATH).await?;
            Modules1::modules_changed(r.signal_emitter()).await
        }
    }
}
