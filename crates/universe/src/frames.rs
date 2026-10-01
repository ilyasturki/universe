use std::collections::{HashMap, VecDeque};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use sha1::{Digest, Sha1};
use tokio::sync::mpsc;
use tokio::task::{AbortHandle, JoinSet};

use crate::paths;

pub const COUNT: usize = 16;
/// The frame standing for the recording: about a fifth in, past the launch and the menus.
pub const THUMB: usize = 3;
const WIDTH: u32 = 640;
// A 4K AV1 frame takes 0.4 s and 180 MB through VAAPI, 1 s, 2.4 s of CPU and 630 MB in software.
const WORKERS: usize = 2;

/// `$XDG_CACHE_HOME/universe/frames/<sha1 of the path>/`: the Qt host keeps its frames there too.
pub fn dir(recording: &Path) -> PathBuf {
    let digest = Sha1::digest(recording.as_os_str().as_bytes());
    paths::cache_home().join("frames").join(digest.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

pub fn file(recording: &Path, index: usize) -> PathBuf {
    dir(recording).join(format!("{index:02}.jpg"))
}

/// Where frame `index` of `COUNT` is taken: the middle of its slice of the recording.
pub fn at(index: usize, duration_s: f64) -> f64 {
    (index as f64 + 0.5) / COUNT as f64 * duration_s
}

fn ffmpeg_args(recording: &Path, seconds: f64, out: &Path, vaapi: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = ["-loglevel", "error", "-y"].map(String::from).into();
    if let Some(device) = vaapi {
        args.extend(["-hwaccel", "vaapi", "-hwaccel_device", device, "-hwaccel_output_format", "vaapi"].map(String::from));
    }
    let scale = if vaapi.is_some() { format!("scale_vaapi=w={WIDTH}:h=-2:format=nv12,hwdownload,format=nv12") } else { format!("scale={WIDTH}:-2") };
    args.extend(["-ss".into(), format!("{seconds:.3}"), "-i".into(), recording.to_string_lossy().into_owned()]);
    args.extend(["-frames:v".into(), "1".into(), "-vf".into(), scale, "-q:v".into(), "4".into(), out.to_string_lossy().into_owned()]);
    args
}

#[derive(Debug, Clone, PartialEq)]
pub struct Landed {
    pub recording: PathBuf,
    pub index: usize,
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Job {
    recording: PathBuf,
    index: usize,
}

enum Command {
    Thumbnail(PathBuf, f64),
    Select(PathBuf, f64),
    Forget(PathBuf),
}

/// Frames sampled from the recordings with ffmpeg, two at a time; each lands in the cache once and is announced then.
#[derive(Debug)]
pub struct Frames {
    commands: mpsc::UnboundedSender<Command>,
    task: tokio::task::JoinHandle<()>,
}

impl Frames {
    /// Runs on the caller's tokio runtime.
    pub fn start() -> (Frames, mpsc::UnboundedReceiver<Landed>) {
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (landed, receiver) = mpsc::unbounded_channel();
        let worker = Worker {
            queue: VecDeque::new(),
            running: HashMap::new(),
            set: JoinSet::new(),
            durations: HashMap::new(),
            hw: None,
            vaapi: crate::gpu::detected().and_then(|g| g.vaapi.clone()),
            ffmpeg: crate::runners::on_path("ffmpeg"),
            landed,
        };
        (Frames { commands, task: tokio::spawn(worker.run(command_rx)) }, receiver)
    }

    /// Queues the recording's `THUMB` frame behind what is already asked for.
    pub fn thumbnail(&self, recording: &Path, duration_s: f64) {
        let _ = self.commands.send(Command::Thumbnail(recording.into(), duration_s));
    }

    /// Every frame of this recording first; another recording's frames still waiting are dropped, its thumbnail kept.
    pub fn select(&self, recording: &Path, duration_s: f64) {
        let _ = self.commands.send(Command::Select(recording.into(), duration_s));
    }

    /// Drops the recording's jobs and its cached frames: the recording is gone.
    pub fn forget(&self, recording: &Path) {
        let _ = self.commands.send(Command::Forget(recording.into()));
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Worker {
    queue: VecDeque<Job>,
    running: HashMap<Job, AbortHandle>,
    set: JoinSet<(Job, bool, bool)>,
    durations: HashMap<PathBuf, f64>,
    /// Whether VAAPI decodes here: unknown until a frame comes through it or the first one fails.
    hw: Option<bool>,
    vaapi: Option<String>,
    ffmpeg: Option<PathBuf>,
    landed: mpsc::UnboundedSender<Landed>,
}

impl Worker {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        loop {
            tokio::select! {
                command = commands.recv() => match command {
                    Some(command) => self.handle(command),
                    None => return,
                },
                Some(done) = self.set.join_next(), if !self.set.is_empty() => {
                    if let Ok((job, ok, hw)) = done {
                        self.finished(job, ok, hw);
                    }
                }
            }
            self.pump();
        }
    }

    fn waiting(&self, job: &Job) -> bool {
        self.queue.contains(job) || self.running.contains_key(job) || file(&job.recording, job.index).is_file()
    }

    fn stop(&mut self, keep: impl Fn(&Job) -> bool) {
        self.running.retain(|job, handle| {
            let kept = keep(job);
            if !kept {
                handle.abort();
            }
            kept
        });
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Thumbnail(recording, duration) => {
                self.durations.insert(recording.clone(), duration);
                let job = Job { recording, index: THUMB };
                if !self.waiting(&job) {
                    self.queue.push_back(job);
                }
            }
            Command::Select(recording, duration) => {
                self.durations.insert(recording.clone(), duration);
                let mine: Vec<Job> = (0..COUNT)
                    .map(|index| Job { recording: recording.clone(), index })
                    .filter(|j| !self.running.contains_key(j) && !file(&j.recording, j.index).is_file())
                    .collect();
                self.queue.retain(|j| j.recording != recording && j.index == THUMB);
                for (i, job) in mine.into_iter().enumerate() {
                    self.queue.insert(i, job);
                }
                self.stop(|j| j.recording == recording || j.index == THUMB);
            }
            Command::Forget(recording) => {
                self.queue.retain(|j| j.recording != recording);
                self.stop(|j| j.recording != recording);
                self.durations.remove(&recording);
                let _ = std::fs::remove_dir_all(dir(&recording));
            }
        }
    }

    fn pump(&mut self) {
        let Some(ffmpeg) = self.ffmpeg.clone() else {
            self.queue.clear();
            return;
        };
        while self.running.len() < WORKERS {
            let Some(job) = self.queue.pop_front() else { return };
            let duration = self.durations.get(&job.recording).copied().unwrap_or(0.0);
            if self.running.contains_key(&job) || duration <= 0.0 || !job.recording.is_file() || file(&job.recording, job.index).is_file() {
                continue;
            }
            let vaapi = self.vaapi.clone().filter(|_| self.hw != Some(false));
            let handle = self.set.spawn(extract(ffmpeg.clone(), job.clone(), at(job.index, duration), vaapi));
            self.running.insert(job, handle);
        }
    }

    fn finished(&mut self, job: Job, ok: bool, hw: bool) {
        if self.running.remove(&job).is_none() {
            return;
        }
        if ok {
            if hw {
                self.hw = Some(true);
            }
            let _ = self.landed.send(Landed { file: file(&job.recording, job.index), recording: job.recording, index: job.index });
        } else if hw && self.hw != Some(true) {
            // The GPU path failed before it ever worked: this run decodes in software, the job goes again.
            self.hw = Some(false);
            self.queue.push_front(job);
        }
    }
}

/// Written under a dot name and renamed into place: a killed extraction leaves no frame that looks whole.
async fn extract(ffmpeg: PathBuf, job: Job, seconds: f64, vaapi: Option<String>) -> (Job, bool, bool) {
    let out = file(&job.recording, job.index);
    let part = out.with_file_name(format!(".{:02}.jpg", job.index));
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let status = tokio::process::Command::new(ffmpeg)
        .args(ffmpeg_args(&job.recording, seconds, &part, vaapi.as_deref()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await;
    let ok = status.is_ok_and(|s| s.success()) && part.is_file() && std::fs::rename(&part, &out).is_ok();
    if !ok {
        let _ = std::fs::remove_file(&part);
    }
    (job, ok, vaapi.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_recording_keeps_the_qt_hosts_cache_layout() {
        let env = paths::test_env();
        let recording = Path::new("/home/u/Videos/universe/Hades 20250101-120000.mkv");
        assert_eq!(file(recording, 3), env.path().join("cache/frames/df9aaa96b8c3e3661cf39527fac094cdd1a3d663/03.jpg"));
        assert_eq!((at(0, 160.0), at(15, 160.0)), (5.0, 155.0));
    }

    #[tokio::test]
    async fn a_selected_recording_lands_every_frame_once() {
        let env = paths::test_env();
        if crate::runners::on_path("ffmpeg").is_none() {
            return;
        }
        let recording = env.path().join("clip.mkv");
        // Past its last frame a seek writes nothing: at 10 fps the last sample of four seconds, 3.875 s, still has one.
        let made = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=320x180:rate=10:duration=4", "-c:v", "mjpeg"])
            .arg(&recording)
            .status()
            .unwrap();
        assert!(made.success());
        let (frames, mut landed) = Frames::start();
        frames.thumbnail(&recording, 4.0);
        frames.select(&recording, 4.0);
        let mut seen = std::collections::BTreeSet::new();
        while seen.len() < COUNT {
            let l = tokio::time::timeout(Duration::from_secs(30), landed.recv()).await.expect("frames land").unwrap();
            assert!(l.file.is_file() && seen.insert(l.index), "frame {} landed twice", l.index);
        }
        let names: Vec<String> = std::fs::read_dir(super::dir(&recording)).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.starts_with('.')), "no part file is left: {names:?}");

        frames.select(&recording, 4.0);
        assert!(tokio::time::timeout(Duration::from_millis(300), landed.recv()).await.is_err(), "nothing is extracted twice");
        frames.forget(&recording);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!super::dir(&recording).exists(), "a forgotten recording's frames go");
    }
}
