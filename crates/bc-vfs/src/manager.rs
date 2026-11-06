//! Provider registry – BC `TVfsManager` / `VfsManagerImpl`.

use crate::{Vfs, VfsError, VfsPath, VfsResult};
use std::sync::Arc;

/// Resolves a [`VfsPath`] to the provider that handles its scheme.
#[derive(Default)]
pub struct VfsManager {
    providers: Vec<Arc<dyn Vfs>>,
}

impl VfsManager {
    /// Empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Manager with only the local provider registered (M1 default).
    pub fn local_only() -> Self {
        let mut m = Self::new();
        m.register(Arc::new(crate::LocalFs::new()));
        m
    }

    /// Register a provider; later registrations win for the same scheme.
    pub fn register(&mut self, provider: Arc<dyn Vfs>) {
        self.providers.push(provider);
    }

    /// Provider for `path`'s scheme.
    pub fn resolve(&self, path: &VfsPath) -> VfsResult<Arc<dyn Vfs>> {
        self.providers
            .iter()
            .rev()
            .find(|p| p.scheme().eq_ignore_ascii_case(path.scheme()))
            .cloned()
            .ok_or_else(|| VfsError::NoProvider(path.scheme().to_string()))
    }

    /// Registered schemes.
    pub fn schemes(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.scheme()).collect()
    }
}

impl std::fmt::Debug for VfsManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsManager")
            .field("schemes", &self.schemes())
            .finish()
    }
}
