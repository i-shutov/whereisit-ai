//! Cooperative cancellation for CTF operations with a single publication boundary.
use atlas_core::{Error, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const RUNNING: u8 = 0;
const CANCELLED: u8 = 1;
const PUBLISHING: u8 = 2;

#[derive(Default)]
struct State {
    status: AtomicU8,
    entries: AtomicU64,
    phase: Mutex<&'static str>,
}

#[derive(Default)]
pub(crate) struct Job {
    id: Option<String>,
    state: Arc<State>,
}

fn registry() -> &'static Mutex<HashMap<String, Arc<State>>> {
    static JOBS: OnceLock<Mutex<HashMap<String, Arc<State>>>> = OnceLock::new();
    JOBS.get_or_init(Mutex::default)
}

fn unavailable() -> Error {
    Error::new("internal", "CTF job state unavailable")
}

fn cancelled() -> Error {
    Error::new("cancelled", "CTF operation cancelled before publication")
}

impl Job {
    #[cfg(test)]
    pub(crate) fn entry_counter(&self) -> impl Fn() -> u64 + Send + Sync + 'static {
        let state = self.state.clone();
        move || state.entries.load(Ordering::Relaxed)
    }

    pub(crate) fn start(v: &Value) -> Result<Self> {
        let mut job = Self::default();
        job.phase("preparing")?;
        if v.get("job_id").is_none() {
            return Ok(job);
        }
        let id = crate::text(v, "job_id")?;
        if id.is_empty() || id.len() > 128 {
            return Err(Error::new(
                "invalid_request",
                "job_id must be 1 to 128 bytes",
            ));
        }
        let mut jobs = registry().lock().map_err(|_| unavailable())?;
        if jobs.contains_key(id) {
            return Err(Error::new("job_exists", "CTF job is already running"));
        }
        jobs.insert(id.to_owned(), job.state.clone());
        job.id = Some(id.to_owned());
        Ok(job)
    }

    pub(crate) fn phase(&self, phase: &'static str) -> Result<()> {
        self.check()?;
        *self.state.phase.lock().map_err(|_| unavailable())? = phase;
        Ok(())
    }

    pub(crate) fn entry(&self) -> Result<()> {
        self.check()?;
        self.state.entries.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn check(&self) -> Result<()> {
        if self.state.status.load(Ordering::Acquire) == CANCELLED {
            Err(cancelled())
        } else {
            Ok(())
        }
    }

    /// Shared checkpoint for parallel native query workers. It shares only the
    /// cancellation/progress state, never the registry-owning Job identifier.
    #[cfg(feature = "duckdb-query")]
    pub(crate) fn observer(&self) -> Arc<dyn Fn(u64) -> Result<()> + Send + Sync> {
        let state = self.state.clone();
        Arc::new(move |entries| {
            if state.status.load(Ordering::Acquire) == CANCELLED {
                return Err(cancelled());
            }
            state.entries.fetch_add(entries, Ordering::Relaxed);
            Ok(())
        })
    }

    pub(crate) fn identity(
        &self,
        path: &std::path::Path,
        max_bytes: u64,
    ) -> Result<crate::ctf_sidecar::SourceIdentity> {
        let identity = crate::ctf_sidecar::identity_with_checkpoint(path, max_bytes, || {
            self.check()
                .map_err(|_| std::io::Error::other("CTF operation cancelled"))
        });
        self.check()?;
        identity.map_err(|e| Error::new("io", format!("CTF source I/O error: {:?}", e.kind())))
    }

    pub(crate) fn begin_publication(&self) -> Result<()> {
        match self.state.status.compare_exchange(
            RUNNING,
            PUBLISHING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => self.phase("publishing"),
            Err(CANCELLED) => Err(cancelled()),
            Err(_) => Ok(()),
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if let (Some(id), Ok(mut jobs)) = (&self.id, registry().lock()) {
            jobs.remove(id);
        }
    }
}

fn find(v: &Value) -> Result<(&str, Arc<State>)> {
    let id = crate::text(v, "job_id")?;
    let jobs = registry().lock().map_err(|_| unavailable())?;
    let state = jobs
        .get(id)
        .cloned()
        .ok_or_else(|| Error::new("not_found", "CTF job is not running"))?;
    Ok((id, state))
}

pub(crate) fn cancel(v: &Value) -> Result<Value> {
    let (_, state) = find(v)?;
    match state
        .status
        .compare_exchange(RUNNING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
    {
        Ok(_) | Err(CANCELLED) => Ok(json!({})),
        Err(_) => Err(Error::new("too_late", "CTF job is already publishing")),
    }
}

pub(crate) fn progress(v: &Value) -> Result<Value> {
    let (id, state) = find(v)?;
    let phase = *state.phase.lock().map_err(|_| unavailable())?;
    Ok(json!({
        "job_id":id,
        "phase":phase,
        "entries":state.entries.load(Ordering::Relaxed),
        "cancelled":state.status.load(Ordering::Acquire) == CANCELLED
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_and_publication_cannot_both_win() {
        let args = json!({"job_id":"ctf-job-state-test"});
        let job = Job::start(&args).unwrap();
        job.phase("importing").unwrap();
        job.entry().unwrap();
        assert_eq!(progress(&args).unwrap()["entries"], 1);
        assert_eq!(progress(&args).unwrap()["phase"], "importing");
        assert_eq!(Job::start(&args).err().unwrap().code, "job_exists");
        cancel(&args).unwrap();
        assert_eq!(job.begin_publication().unwrap_err().code, "cancelled");
        assert_eq!(progress(&args).unwrap()["cancelled"], true);
        drop(job);
        assert_eq!(progress(&args).unwrap_err().code, "not_found");

        let publish = Job::start(&args).unwrap();
        publish.begin_publication().unwrap();
        assert_eq!(cancel(&args).unwrap_err().code, "too_late");
        assert_eq!(progress(&args).unwrap()["phase"], "publishing");
    }
}
