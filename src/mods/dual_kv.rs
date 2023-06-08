use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use std::mem;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use std::{thread, thread::JoinHandle};

use deepsize::DeepSizeOf;
use itertools::Itertools;

// use std::time::Instant;

use napi::bindgen_prelude::*;
use rusqlite::Connection;

fn human(n: u64) -> String {
    n.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .flatten()
        .collect::<Vec<_>>()
        .join(",")
}

fn make_db(path: &String) -> Connection {
    let db = Connection::open(path).unwrap();
    db.execute(
        "CREATE TABLE IF NOT EXISTS
            kv (k TEXT, v TEXT, t CHAR, PRIMARY KEY(k) )
            WITHOUT ROWID",
        (),
    )
    .expect("kv table created");
    let pragmas = "
            PRAGMA page_size = 8196;
            PRAGMA threads = 2;
            PRAGMA journal_mode = OFF;
            PRAGMA locking_mode = EXCLUSIVE;
            PRAGMA wal_autocheckpoint = N;
            PRAGMA synchronous = 0;
            PRAGMA count_changes = OFF;
            PRAGMA journal_mode = OFF;
            PRAGMA temp_store = MEMORY;
            PRAGMA mmap_size = 8000000000;
            PRAGMA cache_size = -50000;
            PRAGMA cache_spill = OFF;
            PRAGMA automatic_index = OFF;
            PRAGMA auto_vacuum = NONE;
            PRAGMA optimize;
        ";
    db.execute_batch(pragmas)
        .expect("failed to execute pragmas");

    db
}

type Key = String;
type Value = String;
type CntValue = (usize, Value);
type Cache = HashMap<Key, CntValue>;
type CacheShare = Arc<Mutex<Cache>>;
type SharedConnection = Arc<Mutex<Connection>>;
type KVIntoIter = std::collections::hash_map::IntoIter<Key, CntValue>;

#[napi]
pub struct DualKV {
    cache: CacheShare,
    db: SharedConnection,
    workers: Vec<JoinHandle<()>>,
    should_stop: Arc<AtomicBool>,
    iter: Option<KVIntoIter>,
}

fn db_worker(
    max_sz: usize,
    batch_size: Option<u32>,
    db_share: SharedConnection,
    cache_share: CacheShare,
    should_stop: Arc<AtomicBool>,
) {
    let batch_size: u32 = batch_size.unwrap_or(4000);
    println!("batch thread started for sqlite with batch {batch_size}");

    let lfu_evict = |c: &Cache, k_evict: usize| {
        c.iter()
            .collect::<Vec<_>>()
            .into_iter()
            .sorted_by_key(|(_, &(kc, _))| kc)
            .rev()
            .take(k_evict)
            .map(|(k, cv)| (k.to_owned(), cv.to_owned()))
            .collect::<Vec<(Key, CntValue)>>()
    };

    let insert_batch = |batch: Vec<(Key, Value)>| {
        let len = batch.len();
        let db = db_share.lock().unwrap();
        let batch_flat: Vec<String> = batch.into_iter().flat_map(|(k, v)| vec![k, v]).collect();
        let values = rusqlite::params_from_iter(&batch_flat);
        let placeholders = (1..=len).map(|_| "(?,?)").collect::<Vec<_>>().join(", ");

        let q = format!("INSERT OR REPLACE INTO kv (k,v) VALUES {}", placeholders);

        let mut stmt = db
            .prepare_cached(&q)
            .map_err(|e| {
                let estr = e.to_string();
                let err_msg = format!(
                    "Error in prepare: {}",
                    (if estr.len() > 200 {
                        estr.chars().take(200).collect::<String>() + "..."
                    } else {
                        estr
                    })
                );
                Err::<String, String>(err_msg)
            })
            .expect("prepare batch insert");

        stmt.execute(values)
            .expect(format!("batch insert {}", len).as_str());
    };

    while !should_stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));

        let cache_sz = cache_share.lock().unwrap().deep_size_of();

        if cache_sz > max_sz {
            println!(
                " ••••• cache ram: {} > {}",
                human(cache_sz as u64),
                human(max_sz as u64)
            );

            let cache = cache_share.lock().unwrap();
            let cache_len = cache.len();
            let k_evict = (cache_len * (cache_sz - max_sz)) / cache_sz;
            if k_evict == 0 {
                continue;
            }

            let inserts = lfu_evict(&cache, k_evict)
                .into_iter()
                .map(|(k, cv)| (k, cv.1))
                .collect::<Vec<(Key, Value)>>();

            drop(cache);

            for chunk in inserts.chunks(batch_size as usize) {
                insert_batch(chunk.to_vec());
            }

            let start = Instant::now();
            let mut cache = cache_share.lock().unwrap();
            for (k, _) in inserts {
                cache.remove(&k);
            }
            cache.shrink_to_fit();
            let t0 = start.elapsed().as_millis();
            drop(cache);
            let cache = cache_share.lock().unwrap();
            let cache_sz = cache.deep_size_of();
            println!(
                " :::: evicted {} in {}ms / cache_len: {} :::::: AFTER: {} <=> {}",
                human(k_evict as u64),
                t0,
                human(cache_len as u64),
                human(cache_sz as u64),
                human(max_sz as u64)
            );
        }
    }

    for it in lfu_evict(&(cache_share.lock().unwrap()), 30) {
        println!("top!! {:?}", it);
    }

    println!("THREAD monitor done.");
}

#[napi]
impl DualKV {
    #[napi(constructor)]
    pub fn new(
        max_sz: i64,
        batch_size: u32,
        threads: Option<u8>,
        path: String,
    ) -> napi::Result<Self> {
        let threads: u8 = threads.unwrap_or(1);

        let cache: CacheShare = Arc::new(Mutex::new(HashMap::new()));
        let db_shared = Arc::new(Mutex::new(make_db(&path)));
        let should_stop = Arc::new(AtomicBool::new(false));
        let mut workers = Vec::new();

        for _ in 1..=threads {
            let should_stop = Arc::clone(&should_stop);
            let db_copy = Arc::clone(&db_shared);
            let cache_copy = Arc::clone(&cache);
            let worker = thread::spawn(move || {
                db_worker(
                    max_sz as usize,
                    Some(batch_size),
                    db_copy,
                    cache_copy,
                    should_stop,
                )
            });
            workers.push(worker);
        }

        Ok(Self {
            iter: None,
            cache,
            workers,
            should_stop,
            db: db_shared,
        })
    }

    #[napi(writable = false)]
    pub fn set(&mut self, key: Key, value: Value) {
        let mut cache = self.cache.lock().unwrap();
        if let Some((_, _)) = cache.insert(key, (0, value)) {
            //panic!("error inserting: {:?}", e);
        }
    }

    #[napi(writable = false)]
    pub fn get(&mut self, key: Key) -> napi::Result<Either<Value, Undefined>> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(cv) = cache.get_mut(&key) {
            cv.0 += 1;
            return Ok(Either::A(cv.1.to_owned()));
        } else {
            let db = self.db.lock().unwrap();
            let mut stmt = db
                .prepare_cached("SELECT v,t FROM kv WHERE k=? LIMIT 1")
                .unwrap();
            let mut rows = stmt.query(&[&key]).unwrap();
            match rows.next() {
                Ok(Some(v)) => {
                    let value = v.get_unwrap::<_, String>(0);
                    cache.insert(key.clone(), (1, value.to_owned()));
                    Ok(Either::A(value))
                }
                _ => Ok(Either::B(() as Undefined)),
            }
        }
    }

    #[napi(writable = false)]
    pub fn for_each(&mut self, env: Env, callback: JsFunction) {
        let cache = self.cache.lock().unwrap();

        for (k, cv) in cache.iter() {
            let k_str = env.create_string(k).unwrap();
            let v_str = env.create_string(cv.1.as_ref()).unwrap();
            callback
                .call::<napi::JsString>(None, &[k_str, v_str])
                .unwrap();
        }
    }

    #[napi(writable = false)]
    pub fn filter_keys(&mut self, env: Env, callback: JsFunction) -> Array {
        let cache = self.cache.lock().unwrap();
        let mut arr = env.create_array(0).unwrap();

        for k in cache.keys() {
            let k_str = env.create_string(k).unwrap();
            let res = callback.call::<napi::JsString>(None, &[k_str]).unwrap();
            match res.coerce_to_bool() {
                Ok(b) => {
                    if b.get_value().unwrap() {
                        arr.insert(k.to_string()).unwrap();
                    }
                }
                Err(_) => (),
            }
        }
        arr
    }

    #[napi(writable = false)]
    pub fn length(&mut self) -> Result<i128> {
        let cache = self.cache.lock().unwrap();
        Ok(cache.len() as i128)
    }

    #[napi(writable = false)]
    pub fn start_iter(&mut self) {
        let cache = self.cache.lock().unwrap();
        self.iter = Some(cache.clone().into_iter());
    }

    #[napi(writable = false)]
    pub fn next(&mut self, env: Env) -> Result<Either<Array, Undefined>> {
        if self.iter.is_none() {
            self.start_iter();
        }

        match self.iter.as_mut().unwrap().next() {
            Some((k, cv)) => {
                let mut arr = env.create_array(2).unwrap();
                arr.set(0, k.to_string()).unwrap();
                arr.set(1, cv.1.to_string()).unwrap();
                Ok(Either::A(arr))
            }
            None => Ok(Either::B(() as Undefined)),
        }
    }

    #[napi(writable = false)]
    pub fn nth(&mut self, env: Env, ix: i64) -> Result<Either<Array, Undefined>> {
        if self.iter.is_none() {
            self.start_iter();
        }

        match self.iter.as_mut().unwrap().nth(ix as usize) {
            Some((k, cv)) => {
                let mut arr = env.create_array(2).unwrap();
                arr.set(0, k.to_string()).unwrap();
                arr.set(1, cv.1.to_string()).unwrap();
                Ok(Either::A(arr))
            }
            None => Ok(Either::B(() as Undefined)),
        }
    }

    #[napi(writable = false)]
    pub fn memory(&mut self) -> i64 {
        let cache = self.cache.lock().unwrap();
        mem::size_of_val(&cache) as i64
    }

    #[napi(writable = false)]
    pub fn entries_iter(&mut self, env: Env) -> napi::JsObject {
        let cache = self.cache.clone();
        let locked_cache = cache.lock().unwrap();
        let kv = Rc::new(RefCell::new(locked_cache.clone().into_iter()));

        let next_js = env.create_function_from_closure("next", move |_| {
            let mut res = env.create_object().unwrap();

            let mut iter = kv.borrow_mut();

            match iter.next() {
                Some((k, cv)) => {
                    let mut arr = env.create_array(2).unwrap();
                    arr.set(0, k.to_string()).unwrap();
                    arr.set(1, cv.1.to_string()).unwrap();
                    res.set::<&str, Array>("value", arr).unwrap();
                    res.set("done", false).unwrap();
                    Ok(res)
                }
                None => {
                    res.set("done", true).unwrap();
                    Ok(res)
                }
            }
        });

        let mut obj = env.create_object().unwrap();
        obj.set("next", next_js).unwrap();
        obj
    }

    #[napi(writable = false)]
    pub fn entries_flat(&mut self) -> Vec<String> {
        let cache = self.cache.lock().unwrap();
        cache
            .iter()
            .map(|(k, cv)| vec![k.to_owned(), cv.1.to_owned()])
            .flatten()
            .collect()
    }

    #[napi(writable = false)]
    pub fn entries2(&mut self, env: Env) -> Result<Array> {
        let cache = self.cache.lock().unwrap();
        let mut arr = env.create_array(cache.len() as u32).unwrap();
        for (k, cv) in cache.iter() {
            arr.insert(k.to_owned()).unwrap();
            arr.insert(cv.1.to_owned()).unwrap();
        }
        Ok(arr)
    }

    #[napi(writable = false)]
    pub fn entries(&mut self) -> Vec<Vec<String>> {
        let cache = self.cache.lock().unwrap();
        cache
            .iter()
            .map(|(k, cv)| vec![k.to_owned(), cv.1.to_owned()])
            .collect()
    }

    #[napi(writable = false)]
    pub fn delete(&mut self, key: Key) {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(&key);
    }

    #[napi(writable = false)]
    pub fn destroy(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        while let Some(worker) = self.workers.pop() {
            worker.join().unwrap();
        }
    }
}

impl Drop for DualKV {
    fn drop(&mut self) {
        self.destroy()
    }
}
