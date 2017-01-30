A cache that will keep track of the total size of the elements put in and evict
based on that value. The cache is fully thread safe and returns Arc references.

Usage
-----

```rust
extern crate multicache;
use multicache::MultiCache;
use std::sync::Arc;

fn main() {
  let cache = MultiCache::new(200);

  cache.put(0, 0, 100);
  cache.put(1, 1, 100);
  cache.put(2, 2, 100);

  assert_eq!(cache.get(0), None);
  assert_eq!(cache.get(1), Some(Arc::new(1)));
  assert_eq!(cache.get(2), Some(Arc::new(2)));
}
```

Doing a get bumps the value to be the last to be evicted:

```rust
extern crate multicache;
use multicache::MultiCache;
use std::sync::Arc;

fn main() {
  let cache = MultiCache::new(200);

  cache.put(0, 0, 100);
  cache.put(1, 1, 100);
  cache.get(0);
  cache.put(2, 2, 100);

  assert_eq!(cache.get(0), Some(Arc::new(0)));
  assert_eq!(cache.get(1), None);
  assert_eq!(cache.get(2), Some(Arc::new(2)));
}
```
