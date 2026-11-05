//! Resource pools – BC `TJobResourcePool` / `TJobResource` / `TJobResourceThread`.
//!
//! A resource is anything with limited concurrency: an FTP connection, a physical disk,
//! an archive handle. Jobs declare the resources they need ([`crate::Job::resources`]);
//! the pool serialises access so e.g. two jobs never hammer one spinning disk.

use std::collections::HashMap;
use std::sync::Mutex;

/// Identifies a resource (`TVfsResource` instance, disk volume, …).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceKey(pub String);

/// Lease on a resource; released on drop.
#[derive(Debug)]
pub struct ResourceLease {
    key: ResourceKey,
}

impl ResourceLease {
    pub fn key(&self) -> &ResourceKey {
        &self.key
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        // scaffold: release back to the pool once implemented
    }
}

/// Capacity-limited resource registry.
#[derive(Debug, Default)]
pub struct ResourcePool {
    limits: Mutex<HashMap<ResourceKey, usize>>,
}

impl ResourcePool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a resource with a maximum number of simultaneous leases.
    pub fn declare(&self, key: ResourceKey, max_concurrent: usize) {
        if let Ok(mut limits) = self.limits.lock() {
            limits.insert(key, max_concurrent);
        }
    }

    /// Block until a lease on every key is available (deadlock-free ordering).
    pub fn acquire(&self, keys: &[ResourceKey]) -> Vec<ResourceLease> {
        todo!("bc-jobs::resource: acquire leases in canonical order")
    }

    /// Non-blocking variant.
    pub fn try_acquire(&self, keys: &[ResourceKey]) -> Option<Vec<ResourceLease>> {
        todo!("bc-jobs::resource: try acquire")
    }
}
