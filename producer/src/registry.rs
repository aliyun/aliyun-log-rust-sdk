use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// Cached destination identity. Holds no producer handle, avoiding an Arc cycle.
pub(crate) struct WriterTarget {
    pub index: usize,
    pub project: Arc<str>,
    pub logstore: Arc<str>,
}

/// Append-only destination cache. Writer lookup and sealed batches take a lock;
/// individual sends keep using the index cached in their writer.
#[derive(Default)]
pub(crate) struct WriterCache {
    state: RwLock<CacheState>,
}

#[derive(Default)]
struct CacheState {
    projects: HashMap<Arc<str>, HashMap<Arc<str>, Arc<WriterTarget>>>,
    targets: Vec<Arc<WriterTarget>>,
}

impl CacheState {
    fn get(&self, project: &str, logstore: &str) -> Option<&Arc<WriterTarget>> {
        self.projects.get(project)?.get(logstore)
    }
}

impl WriterCache {
    pub fn get_or_create(&self, project: &str, logstore: &str) -> Arc<WriterTarget> {
        if let Some(target) = self.state.read().unwrap().get(project, logstore) {
            return target.clone();
        }
        let mut state = self.state.write().unwrap();
        // Concurrent first lookups must publish one identity.
        if let Some(target) = state.get(project, logstore) {
            return target.clone();
        }
        let target = Arc::new(WriterTarget {
            index: state.targets.len(),
            project: project.into(),
            logstore: logstore.into(),
        });
        state
            .projects
            .entry(target.project.clone())
            .or_default()
            .insert(target.logstore.clone(), target.clone());
        state.targets.push(target.clone());
        target
    }

    pub fn by_index(&self, index: usize) -> Arc<WriterTarget> {
        self.state.read().unwrap().targets[index].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_lookups_reuse_one_identity_without_merging_destinations() {
        let cache = Arc::new(WriterCache::default());
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let cache = cache.clone();
                std::thread::spawn(move || cache.get_or_create("project", "store"))
            })
            .collect();
        let targets: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert!(targets
            .iter()
            .all(|target| Arc::ptr_eq(target, &targets[0])));
        let other_project = cache.get_or_create("other", "store");
        let other_logstore = cache.get_or_create("project", "other");
        assert_ne!(targets[0].index, other_project.index);
        assert_ne!(other_project.index, other_logstore.index);
        let index = targets[0].index;
        drop(targets);
        assert!(Arc::ptr_eq(
            &cache.by_index(index),
            &cache.get_or_create("project", "store")
        ));
    }
}
