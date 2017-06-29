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
//!    assert_eq!(cache.get(0), None);
//!    assert_eq!(cache.get(1), Some(Arc::new(1)));
//!    assert_eq!(cache.get(2), Some(Arc::new(2)));
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
//!    cache.get(0);
//!    cache.put(2, 2, 100);
//!
//!    assert_eq!(cache.get(0), Some(Arc::new(0)));
//!    assert_eq!(cache.get(1), None);
//!    assert_eq!(cache.get(2), Some(Arc::new(2)));
//!  }
//! ```

extern crate linked_hash_map;
use linked_hash_map::LinkedHashMap;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, Arc};
use std::fmt;

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
  aliases: HashMap<K,K>,
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
        aliases: HashMap::new(),
        totalsize: 0,
        maxsize: bytesize,
      }),
    }
  }

  /// Add a new element by key/value with a given bytesize, if after inserting this
  /// element we would be going over the bytesize of the cache first enough elements are
  /// evicted for that to not be the case
  pub fn put(&self, key: K, value: V, bytes: usize) 
  where K: Hash+Eq+Copy {
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

  /// Get an element from the cache, updating it so it's now the most recently used and
  /// thus the last to be evicted
  pub fn get(&self, key: K) -> Option<Arc<V>>
  where K: Hash+Eq+Copy {
    let alias = {
      let mut mparts = self.parts.lock().unwrap();
      if let Some(val) = mparts.hash.get_refresh(&key) {
        return Some(val.val.clone())
      }

      // If direct failed try an alias
      if let Some(val) = mparts.aliases.get(&key) {
        Some(*val)
      } else {
        None
      }
    };

    if let Some(val) = alias {
      return self.get(val)
    }

    None
  }

  /// Alias a new key to an existing one
  pub fn alias(&self, existing: K, newkey: K)
  where K: Hash+Eq+Copy {
    if existing == newkey {
      return
    }

    // FIXME: aliases are never evicted. A better design would be to make MultiCacheItem
    //        an enum that can both be a cache entry and an alias and then do normal eviction

    let mut mparts = self.parts.lock().unwrap();
    (*mparts).aliases.insert(newkey, existing);
  }

  /// Check if a given key exists in the cache
  pub fn contains_key(&self, key: &K) -> bool
  where K: Hash+Eq+Copy {
    let mparts = self.parts.lock().unwrap();
    if (*mparts).hash.contains_key(&key) {
      return true
    }
    if let Some(newkey) = (*mparts).aliases.get(key) {
      return (*mparts).hash.contains_key(&newkey)
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

  #[test]
  fn aliases() {
    let cache = MultiCache::new(200);

    cache.put(0, 0, 100);
    cache.alias(0, 1);
    cache.put(2, 2, 100);

    assert_eq!(cache.get(0), Some(Arc::new(0)));
    assert_eq!(cache.get(1), Some(Arc::new(0)));
    assert_eq!(cache.get(2), Some(Arc::new(2)));
  }

  #[test]
  fn contains() {
    let cache = MultiCache::new(100);

    cache.put(0, 0, 100);
    cache.alias(0, 1);

    assert_eq!(cache.contains_key(&0), true);
    assert_eq!(cache.contains_key(&1), true);
    assert_eq!(cache.contains_key(&2), false);

    cache.put(2, 2, 100);

    assert_eq!(cache.contains_key(&0), false);
    assert_eq!(cache.contains_key(&1), false);
    assert_eq!(cache.contains_key(&2), true);
  }
}
