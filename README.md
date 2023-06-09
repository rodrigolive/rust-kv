# RustKV

<img
  src="/media/logo.png"
  alt="Rust-KV logo"
  title="Rust-KV logo generated by Bing Images"
  style="display: inline-block; margin: 0 auto; width: 150px;">

A write-behind
[1](<https://en.wikipedia.org/wiki/Cache_(computing)#WRITE-BEHIND>) key-value
store. NodeJS `Map()` on SQLite steroids.

### Description

This is a NodeJS module implementing a KV store with 2 main components
interlinked:

-   A LFU in-memory cache stored in a fast, O(1) Rust [HashMap](https://doc.rust-lang.org/std/collections/struct.HashMap.html)
-   A performanced-tuned SQLite backend based on [rusqlite](https://docs.rs/rusqlite/0.29.0/rusqlite/)

This is the logic behind the implementation:

-   When creating the KV store `new RustKV()`, the user sets the MAX aprox RAM size for the
    HashMap
-   Key-value pairs are inserted first into the Rust HashMap structure
-   With every `get(key)` call, a frequency counter is increased in memory
-   If the HashMap becomes larger than MAX, least-frequent keys-values are
    evicted into the SQLite database and the HashMap is shrunk to MAX

### Motivation

-   We had to store absolutely _huge_ key-value data from many databases, +100M
    keys.
-   We wanted `Map()` or Object _performance_ for the most frequently used keys.
-   We wanted an in-process solution, not another server. Anyway, memcached and
    redis performance are not at par with in-process solutions anyway. Same
    applies for networked databases.
-   Storing key-values in NodeJS (V8) can be done with a `new Map()` but a `Map`
    has some limitations: the heap size of NodeJS is typically 4GB and the
    `Map`can only take in so many entries, so it needs some type of
    rehashing/sharding (maps of maps).
-   Setting the max _heap size_ in Node dynamically is not possible, and
    sometimes inconvenient as it's a CLI parameter (`max_old_space_size`) or ENV
    variable.
-   Any NodeJS/V8 heap data structure will fight for space with other NodeJS data
    structures.
-   GC cleanup of huge Node heap structures can slow everything down.
-   So we moved everything to Rust.

### Architecture and considerations

-   The Rust heap is unbounded, limited only by the server's RAM and the corresponding OS settings.
-   SQLite was chosen because, once fine-tuned, it's as fast or faster than any other Rust
    KV-stores, including LevelDB and Sled, with a much simpler surface and a single file.
-   The SQLite backend _does not store all the key-values_, only the ones evicted from RAM.
    So this library is NOT a persistence store and offers no persistence guarantees.
-   It's _not async_ the get/set calls are _blocking_ because they are FAST
    in the same way as `new Map().set(k,v)` is fast.
-   We chose LFU (instead of, say, LRU) because it fits our use case (databases) better.

## Usage

```javascript
const { RustKV } = require('rust-kv');

// 8GB RAM HashMap, then less frequent kvs stored in SQLite
const kv = new RustKV({
    db: '/tmp/cache.db',
    maxMem: 8 * 1024 ** 3
});
kv.set('foo', 'bar');
const value = kv.get('foo');

// no SQLite backend, all in RAM
const kvMemOnly = new RustKV();
```
