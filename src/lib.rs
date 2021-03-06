//! A cache that will keep track of the total size of the elements put in and evict
//! based on that value. The cache is fully thread safe and returns Arc references.
//!
//! # Example
//! ```rust
//!  extern crate multicache;
//!  use multicache::MultiCache;
//!  use std::sync::Arc;
//!
//!  fn main() {
//!    let cache = MultiCache::new(200);
//!
//!    cache.put(0, 0, 100);
//!    cache.put(1, 1, 100);
//!    cache.put(2, 2, 100);
//!
//!    assert_eq!(cache.get(&0), None);
//!    assert_eq!(cache.get(&1), Some(Arc::new(1)));
//!    assert_eq!(cache.get(&2), Some(Arc::new(2)));
//!  }
//! ```
//!
//! Doing a get bumps the value to be the last to be evicted:
//!
//! ```rust
//!  extern crate multicache;
//!  use multicache::MultiCache;
//!  use std::sync::Arc;
//!
//!  fn main() {
//!    let cache = MultiCache::new(200);
//!
//!    cache.put(0, 0, 100);
//!    cache.put(1, 1, 100);
//!    cache.get(&0);
//!    cache.put(2, 2, 100);
//!
//!    assert_eq!(cache.get(&0), Some(Arc::new(0)));
//!    assert_eq!(cache.get(&1), None);
//!    assert_eq!(cache.get(&2), Some(Arc::new(2)));
//!  }
//! ```

extern crate linked_hash_map;
use linked_hash_map::LinkedHashMap;
use std::hash::Hash;
use std::sync::{Mutex, Arc};
use std::fmt;

struct MultiCacheItem<V> {
  val: V,
  bytes: usize,
}

impl<V> MultiCacheItem<V> {
  pub fn new(val: Arc<V>, bytes: usize) -> MultiCacheItem<Arc<V>> {
    MultiCacheItem {
      val: val,
      bytes: bytes,
    }
  }
}

struct MultiCacheParts<K,V> {
  hash: LinkedHashMap<K,MultiCacheItem<Arc<V>>>,
  totalsize: usize,
  maxsize: usize,
}

impl<K,V> fmt::Debug for MultiCacheParts<K,V> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{{ {} totalsize, {} maxsize }}",
      self.totalsize, self.maxsize)
  }
}

#[derive(Debug)]
pub struct MultiCache<K,V> {
  parts: Mutex<MultiCacheParts<K,V>>,
}

impl<K,V> MultiCache<K,V> {
  /// Create a new cache which will at most hold a total of bytesize in elements
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

  /// Add a new element by key/value with a given bytesize, if after inserting this
  /// element we would be going over the bytesize of the cache first enough elements are
  /// evicted for that to not be the case
  pub fn put(&self, key: K, value: V, bytes: usize) 
  where K: Hash+Eq {
    self.put_arc(key, Arc::new(value), bytes)
  }

  /// Add a new element by key/Arc<value> with a given bytesize, if after inserting this
  /// element we would be going over the bytesize of the cache first enough elements are
  /// evicted for that to not be the case
  pub fn put_arc(&self, key: K, value: Arc<V>, bytes: usize) 
  where K: Hash+Eq {
    // First remove this key if it exists already, reclaiming that space
    self.remove(&key);

    let mut mparts = self.parts.lock().unwrap();

    // Now if we still need it reclaim more space
    while mparts.totalsize + bytes > mparts.maxsize {
      match mparts.hash.pop_front() {
        None => break, // probably even the only item is larger than the max
        Some(val) => {
          mparts.totalsize -= val.1.bytes;
        }
      }
    }

    // Finally save the value and take up the space
    (*mparts).hash.insert(key, MultiCacheItem::new(value,bytes));
    mparts.totalsize += bytes;
  }

  /// Get an element from the cache, updating it so it's now the most recently used and
  /// thus the last to be evicted
  pub fn get(&self, key: &K) -> Option<Arc<V>>
  where K: Hash+Eq {
    let mparts = &mut *(self.parts.lock().unwrap());

    if let Some(val) = mparts.hash.get_refresh(key) {
      return Some(val.val.clone())
    }

    None
  }

  /// Remove an element from the cache, returning it if it exists
  pub fn remove(&self, key: &K) -> Option<Arc<V>>
  where K: Hash+Eq {
    let mut mparts = self.parts.lock().unwrap();

    // First remove this key if it exists already, reclaiming that space
    if let Some(val) = (*mparts).hash.remove(&key) {
      mparts.totalsize -= val.bytes;
      Some(val.val)
    } else {
      None
    }
  }

  /// Check if a given key exists in the cache
  pub fn contains_key(&self, key: &K) -> bool
  where K: Hash+Eq {
    let mparts = self.parts.lock().unwrap();
    if (*mparts).hash.contains_key(&key) {
      return true
    }

    false
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

    assert_eq!(cache.get(&2), Some(Arc::new(2)));
    assert_eq!(cache.get(&1), Some(Arc::new(1)));
    assert_eq!(cache.get(&0), None);
  }

  #[test]
  fn evicts_no_repeats() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);
    cache.put(1, 1, 100);
    cache.put(1, 2, 100);
    cache.put(1, 3, 100);

    assert_eq!(cache.get(&1), Some(Arc::new(3)));
    assert_eq!(cache.get(&0), Some(Arc::new(0)));
  }

  #[test]
  fn get_refreshes() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);
    cache.put(1, 1, 100);
    cache.get(&0);
    cache.put(2, 2, 100);

    assert_eq!(cache.get(&0), Some(Arc::new(0)));
    assert_eq!(cache.get(&1), None);
    assert_eq!(cache.get(&2), Some(Arc::new(2)));
  }

  #[test]
  fn contains() {
    let cache = MultiCache::new(100);

    cache.put(0, 0, 100);

    assert_eq!(cache.contains_key(&0), true);
    assert_eq!(cache.contains_key(&2), false);

    cache.put(2, 2, 100);

    assert_eq!(cache.contains_key(&0), false);
    assert_eq!(cache.contains_key(&2), true);
  }

  #[test]
  fn puts() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);
    cache.put_arc(1, Arc::new(1), 100);

    assert_eq!(cache.get(&0), Some(Arc::new(0)));
    assert_eq!(cache.get(&1), Some(Arc::new(1)));
  }

  #[test]
  fn removes() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);

    assert_eq!(cache.remove(&0), Some(Arc::new(0)));
    assert_eq!(cache.remove(&0), None);
    assert_eq!(cache.get(&0), None);
  }
}
