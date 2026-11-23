use atlas_core::{analytics::AnalyticsRequest, Catalog, Error, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, OnceLock,
    },
};

const PREPARED: u8 = 0;
const RUNNING: u8 = 1;
struct Job {
    cancelled: Arc<AtomicBool>,
    interrupt: Mutex<Option<rusqlite::InterruptHandle>>,
    state: AtomicU8,
}
static JOBS: OnceLock<Mutex<HashMap<String, Arc<Job>>>> = OnceLock::new();
fn jobs() -> &'static Mutex<HashMap<String, Arc<Job>>> {
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("invalid_request", format!("{k} must be a string")))
}
fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 128 {
        Err(Error::new(
            "invalid_request",
            "job_id length must be 1..128",
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn prepare(v: &Value) -> Result<Value> {
    let id = field(v, "job_id")?;
    validate_id(id)?;
    let mut g = jobs()
        .lock()
        .map_err(|_| Error::new("internal", "analytics registry poisoned"))?;
    if g.contains_key(id) {
        return Err(Error::new("conflict", "analytics job_id is already active"));
    }
    if g.len() >= 256 {
        return Err(Error::new("busy", "too many analytics jobs are active"));
    }
    g.insert(
        id.into(),
        Arc::new(Job {
            cancelled: Arc::new(AtomicBool::new(false)),
            interrupt: Mutex::new(None),
            state: AtomicU8::new(PREPARED),
        }),
    );
    Ok(json!({"prepared":true,"job_id":id}))
}
pub(crate) fn cancel(v: &Value) -> Result<Value> {
    let id = field(v, "job_id")?;
    let g = jobs()
        .lock()
        .map_err(|_| Error::new("internal", "analytics registry poisoned"))?;
    let job = g
        .get(id)
        .ok_or_else(|| Error::new("not_found", "analytics job not found"))?;
    job.cancelled.store(true, Ordering::Release);
    if let Ok(h) = job.interrupt.lock() {
        if let Some(h) = h.as_ref() {
            h.interrupt();
        }
    }
    Ok(json!({"cancelled":true,"job_id":id}))
}
pub(crate) fn run(v: &Value) -> Result<Value> {
    let id = field(v, "job_id")?;
    validate_id(id)?;
    let job = jobs()
        .lock()
        .map_err(|_| Error::new("internal", "analytics registry poisoned"))?
        .get(id)
        .cloned()
        .ok_or_else(|| Error::new("not_found", "analytics job was not prepared"))?;
    if job
        .state
        .compare_exchange(PREPARED, RUNNING, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(Error::new("conflict", "analytics job is already running"));
    }
    let _cleanup = Cleanup(id.to_owned());
    (|| {
        if job.cancelled.load(Ordering::Acquire) {
            return Err(Error::new(
                "cancelled",
                "analytics request cancelled before opening catalog",
            ));
        }
        let db = field(v, "db")?;
        let request: AnalyticsRequest =
            serde_json::from_value(v.clone()).map_err(|e| Error::new("invalid_request", e))?;
        let c = Catalog::open_readonly(db)?;
        {
            let mut h = job
                .interrupt
                .lock()
                .map_err(|_| Error::new("internal", "analytics interrupt lock poisoned"))?;
            *h = Some(c.conn().get_interrupt_handle());
        }
        if job.cancelled.load(Ordering::Acquire) {
            return Err(Error::new(
                "cancelled",
                "analytics request cancelled before querying",
            ));
        }
        let response = c
            .analytics(&request, || job.cancelled.load(Ordering::Acquire))
            .map_err(|e| {
                if job.cancelled.load(Ordering::Acquire) {
                    Error::new("cancelled", "analytics request cancelled")
                } else {
                    e
                }
            })?;
        if job.cancelled.load(Ordering::Acquire) {
            return Err(Error::new(
                "cancelled",
                "analytics request cancelled before publication",
            ));
        }
        serde_json::to_value(response).map_err(|e| Error::new("internal", e))
    })()
}
struct Cleanup(String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Ok(mut g) = jobs().lock() {
            g.remove(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(1);
    fn id() -> String {
        format!(
            "analytics-service-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }
    #[test]
    fn prepare_cancel_and_run_cleanup_are_race_safe() {
        let job = id();
        prepare(&json!({"job_id":job})).unwrap();
        cancel(&json!({"job_id":job})).unwrap();
        let err = run(&json!({"job_id":job,"db":"does-not-open.sqlite"})).unwrap_err();
        assert_eq!(err.code, "cancelled");
        prepare(&json!({"job_id":job})).unwrap();
        let err = run(&json!({"job_id":job})).unwrap_err();
        assert_eq!(err.code, "invalid_request");
        prepare(&json!({"job_id":job})).unwrap();
    }
}
