extern crate linked_hash_map;
use linked_hash_map::LinkedHashMap;
use std::hash::Hash;
use std::sync::{Mutex, Arc};

struct MultiCacheItem<V> {
  val: V,
  bytes: usize,
}

impl<V> MultiCacheItem<V> {
  pub fn new(val: V, bytes: usize) -> MultiCacheItem<Arc<V>> {
    MultiCacheItem {
      val: Arc::new(val),
      bytes: bytes,
    }
  }
}

struct MultiCacheParts<K,V> {
  hash: LinkedHashMap<K,MultiCacheItem<Arc<V>>>,
  totalsize: usize,
  maxsize: usize,
}

pub struct MultiCache<K,V> {
  parts: Mutex<MultiCacheParts<K,V>>,
}

impl<K,V> MultiCache<K,V> {
  pub fn new(bytesize: usize) -> MultiCache<K,V> 
  where K: Hash+Eq {
    MultiCache {
      parts: Mutex::new(MultiCacheParts{
        hash: LinkedHashMap::new(),
        totalsize: 0,
        maxsize: bytesize,
      }),
    }
  }

  pub fn put(&self, key: K, value: V, bytes: usize) 
  where K: Hash+Eq {
    let mut mparts = self.parts.lock().unwrap();
    while mparts.totalsize + bytes > mparts.maxsize {
      match mparts.hash.pop_front() {
        None => break, // probably even the only item is larger than the max
        Some(val) => {
          mparts.totalsize -= val.1.bytes;
        }
      }
    }
    (*mparts).hash.insert(key, MultiCacheItem::new(value,bytes));
    mparts.totalsize += bytes;
  }

  pub fn get(&self, key: K) -> Option<Arc<V>>
  where K: Hash+Eq {
    let mut mparts = self.parts.lock().unwrap();
    if let Some(val) = mparts.hash.get_refresh(&key) {
      return Some(val.val.clone())
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use super::MultiCache;
  use std::sync::Arc;

  #[test]
  fn evicts() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);
    cache.put(1, 1, 100);
    cache.put(2, 2, 100);

    assert_eq!(cache.get(2), Some(Arc::new(2)));
    assert_eq!(cache.get(1), Some(Arc::new(1)));
    assert_eq!(cache.get(0), None);
  }

  #[test]
  fn get_refreshes() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);
    cache.put(1, 1, 100);
    cache.get(0);
    cache.put(2, 2, 100);

    assert_eq!(cache.get(0), Some(Arc::new(0)));
    assert_eq!(cache.get(1), None);
    assert_eq!(cache.get(2), Some(Arc::new(2)));
  }
}
