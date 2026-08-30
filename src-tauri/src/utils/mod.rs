use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_id(prefix: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("{}_{}_{}", prefix, id, n)
}
