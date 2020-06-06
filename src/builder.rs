use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::cache::Cache;
use crate::policies::CachePolicy;

pub struct CacheBuilder<'l, Key, Value>
    where Key: Hash + Eq {
    policy: CachePolicy,
    size: Option<u64>,
    capacity: Option<usize>,
    on_eviction: Option<Arc<dyn Fn(&Key, &Value) -> () + 'l>>,
}

impl<'l, Key, Value> Default for CacheBuilder<'l, Key, Value>
    where Key: Hash + Eq + Clone,
          Value: Clone
{
    fn default() -> Self {
        CacheBuilder::new()
    }
}

impl<'l, Key, Value> Clone for CacheBuilder<'l, Key, Value>
    where Key: Hash + Eq + Clone,
          Value: Clone
{
    fn clone(&self) -> CacheBuilder<'l, Key, Value> {
        CacheBuilder {
            policy: self.policy,
            size: self.size,
            capacity: self.capacity,
            on_eviction: self.on_eviction.clone(),
        }
    }
}


impl<'l, Key, Value> CacheBuilder<'l, Key, Value>
    where Key: Hash + Eq + Clone,
          Value: Clone
{
    pub fn new() -> CacheBuilder<'l, Key, Value> {
        CacheBuilder {
            policy: CachePolicy::LFU,
            size: None,
            capacity: None,
            on_eviction: None,
        }
    }

    pub fn set_policy(mut self, policy: CachePolicy) -> CacheBuilder<'l, Key, Value> {
        self.policy = policy;
        self
    }

    pub fn set_max_size(mut self, size: u64) -> CacheBuilder<'l, Key, Value> {
        self.size = Some(size);
        self
    }

    pub fn set_max_capacity(mut self, capacity: usize) -> CacheBuilder<'l, Key, Value> {
        self.capacity = Some(capacity);
        self
    }

    pub fn on_eviction<F>(mut self, handler: F) -> CacheBuilder<'l, Key, Value>
        where F: Fn(&Key, &Value) -> () + 'l {
        self.on_eviction = Some(Arc::new(handler));
        self
    }

    pub fn build(&self) -> Cache<'l, Key, Value> {
        Cache {
            capacity: self.capacity,
            max_size: self.size,
            cur_size: 0,
            elements: self.capacity.map_or_else(HashMap::new, HashMap::with_capacity),
            frequencies: HashMap::new(),
            min_frequency: 0,
            age: 0,
            policy: self.policy,
            on_eviction: self.on_eviction.as_ref().cloned(),
        }
    }
}
