use crate::cache::Item;

#[derive(Copy, Clone)]
pub enum CachePolicy {
    LFU,
    LFUDA,
    GDSF,
}

pub(crate) fn calculate_policy<Value: Clone>(policy: CachePolicy, element: &Item<Value>, cache_age: u64) -> u64 {
    match policy {
        CachePolicy::LFU => element.hits,
        CachePolicy::LFUDA => element.hits + cache_age,
        CachePolicy::GDSF => (element.hits / element.weight) + cache_age,
    }
}
