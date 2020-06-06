use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::policies::{CachePolicy, calculate_policy};

#[derive(Clone)]
pub struct Cache<'l, Key: Hash + Eq + Clone, Value: Clone> {
    pub(crate) capacity: Option<usize>,
    pub(crate) max_size: Option<u64>,
    pub(crate) cur_size: u64,
    pub(crate) elements: HashMap<Arc<Key>, Item<Value>>,
    pub(crate) frequencies: HashMap<u64, HashSet<Arc<Key>>>,
    pub(crate) min_frequency: u64,
    pub(crate) age: u64,
    pub(crate) policy: CachePolicy,
    pub(crate) on_eviction: Option<Arc<dyn Fn(&Key, &Value) -> () + 'l>>,
}

#[derive(Clone)]
pub(crate) struct Item<Value: Clone> {
    pub(crate) value: Value,
    pub(crate) weight: u64,
    pub(crate) hits: u64,
    pub(crate) priority_key: u64,
    pub(crate) creation_time: Option<Instant>,
    pub(crate) ttl: Option<Duration>,
}

impl<'l, Key, Value> Cache<'l, Key, Value>
    where Key: Hash + Eq + Clone,
          Value: Clone
{
    fn freq_remove_entry(&mut self, place: u64, key: &Key) {
        if let Some(bucket) = self.frequencies.get_mut(&place) {
            bucket.remove(key);
            if bucket.is_empty() {
                self.frequencies.remove(&place);
            }
        }
    }

    fn check_ttl(&mut self, key: &Key) -> Option<Value> {
        let item = self.elements.get(key)?;
        let i_priority = item.priority_key;
        if item.creation_time?.elapsed() > item.ttl? {
            self.freq_remove_entry(i_priority, &key);
            return self.elements.remove(key).map(|x| x.value);
        }
        None
    }

    fn evict(&mut self) {
        // it definitely exists and have at least 1 element
        let min_f_key = self
            .frequencies.get_mut(&self.min_frequency)
            .unwrap().iter().next().unwrap().clone();

        let item = self.elements.get(&min_f_key).unwrap();
        if self.age < item.priority_key {
            self.age = item.priority_key
        }

        self.remove(&min_f_key);
    }

    fn increment(&mut self, key: &Arc<Key>) {
        let item = self.elements.get_mut(key).unwrap();
        let old_priority = item.priority_key;

        item.hits += 1;
        item.priority_key = calculate_policy(self.policy, item, self.age);

        // old priority was minimal and we deleted the bucket
        if self.min_frequency == old_priority && !self.frequencies.contains_key(&old_priority) {
            self.min_frequency = item.priority_key;
        }

        // move to new bucket - either existing or create one
        self.frequencies.entry(item.priority_key).or_default().insert(key.clone());

        // remove from previous place
        self.freq_remove_entry(old_priority, &key);
    }

    pub fn contains(&mut self, key: &Key) -> bool {
        self.check_ttl(key);
        self.elements.contains_key(key)
    }

    pub fn peek(&mut self, key: &Key) -> Option<&Value> {
        self.check_ttl(key);
        self.elements.get(key).map(|x| &x.value)
    }

    pub fn peek_mut(&mut self, key: &Key) -> Option<&mut Value> {
        self.check_ttl(key);
        self.elements.get_mut(key).map(|x| x.value.borrow_mut())
    }

    pub fn remove_expired(&mut self) {
        let keys: Vec<Key> = self.elements.keys().map(|x| (**x).clone()).collect();
        for key in keys {
            self.check_ttl(&key);
        }
    }

    pub fn len(&self) -> usize {
        // TODO: WARNING about expired ttl and that len() could be bigger than not-expired elements count
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        // TODO: WARNING about expired ttl and is_empty does not delete expired
        self.len() == 0
    }

    pub fn size(&self) -> u64 {
        // TODO: Warning that size could be wrong because of expired
        self.cur_size
    }

    pub fn clear_without_eviction(&mut self) {
        self.elements.clear();
        self.frequencies.clear();
        self.cur_size = 0;
        self.age = 0;
    }

    pub fn clear_with_eviction(&mut self) {
        match &self.on_eviction {
            None => {}
            Some(evict_handler) => {
                for elem in &self.elements {
                    evict_handler(&elem.0, &elem.1.value)
                }
            }
        };
        self.elements.clear();
        self.frequencies.clear();
        self.cur_size = 0;
        self.age = 0;
    }

    pub fn keys(&self) -> impl Iterator<Item=&Key> {
        // TODO: warning about ttl
        self.elements.keys().map(|x| x.as_ref())
    }

    pub fn age(&self) -> u64 { self.age }

    pub fn get(&mut self, key: &Key) -> Option<&Value> {
        self.check_ttl(key);
        let key = self.elements.get_key_value(key).map(|(key, _)| key.clone())?;
        self.increment(&key);
        self.elements.get_mut(&key).map(|result| &result.value)
    }

    pub fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        self.check_ttl(key);
        let key = self.elements.get_key_value(key).map(|(key, _)| key.clone())?;
        self.increment(&key);
        self.elements.get_mut(&key).map(|result| &mut result.value)
    }

    pub fn insert(&mut self, key: Key, value: Value, weight: u64, ttl: Option<Duration>) -> Result<(), &'static str> {
        let key = Arc::new(key);

        // check max_size and size of the object and evist until there's enough free space
        if let Some(max_size) = self.max_size {
            if weight > max_size {
                return Err("Weight of the item is bigger than max_size of the cache.");
            }

            // if element with such key exists we should not take into account current weight of this element
            let existing_elem_weight = self.elements.get(&key).map(|x| x.weight).unwrap_or(0);

            // get more free space
            while (self.cur_size - existing_elem_weight) + weight > max_size {
                self.evict();
            }
        };

        // now we have enough space
        if let Some(item) = self.elements.get_mut(&key) {
            item.weight = weight;
            item.value = value;
            item.ttl = ttl;
            item.creation_time = ttl.and(Some(Instant::now()));
            self.increment(&key);
            self.cur_size += weight;
            return Ok(());
        }

        // check capacity
        if let Some(capacity) = self.capacity {
            while self.len() >= capacity {
                self.evict()
            }
        }

        // create and insert new item
        let item = Item {
            value,
            weight,
            hits: 0,
            priority_key: 0,
            creation_time: ttl.and(Some(Instant::now())),
            ttl,
        };
        self.elements.insert(key.clone(), item);
        self.cur_size += weight;
        self.increment(&key);
        Ok(())
    }

    pub fn remove(&mut self, key: &Key) -> Option<Value> {
        let item = self.elements.get(key)?;

        if let Some(x) = self.on_eviction.as_ref() { x(key, &item.value) }
        self.cur_size -= item.weight;
        let x = item.priority_key;
        self.freq_remove_entry(x, key);
        self.elements.remove(key).map(|x| x.value)
    }
}
