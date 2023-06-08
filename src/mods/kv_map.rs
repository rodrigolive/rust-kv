use napi::bindgen_prelude::*;
use std::collections::HashMap;

type Key = String;
type Value = String;
type Cache = HashMap<Key, Value>;
type KVIntoIter = std::collections::hash_map::IntoIter<Key, Value>;

#[napi]
pub struct KVMap {
    //cache: Cache<String, String, core::hash::BuildHasherDefault<ahash::AHasher>>,
    //pool: ThreadPool,
    //bucket: Bucket<'static, String, String>,
    map: Cache,
    iter: Option<KVIntoIter>,
}

#[napi]
impl KVMap {
    #[napi(constructor)]
    pub fn new() -> napi::Result<Self> {
        // let cfg = Config::new(path);
        // let store = Store::new(cfg).unwrap();
        // let bucket = store.bucket::<String, String>(Some("aaa")).unwrap();
        //let size = 4;
        //let pool = ThreadPool::new(size);
        // let mut cache = Cache::builder()
        //     .weigher(|_key, value: &String| -> u32 { value.len().try_into().unwrap_or(u32::MAX) })
        //     .max_capacity(sz.into())
        //     .build_with_hasher(hashbrown::hash_map::DefaultHashBuilder::default());
        //let (tx, rx) = channel();

        Ok(Self {
            //cache,
            //pool,
            //bucket,
            iter: None,
            map: HashMap::new(),
        })
    }

    // writable / enumerable / configurable
    #[napi(writable = false)]
    pub fn set(&mut self, key: String, value: String) {
        // self.bucket.set(&k, &v).unwrap();
        //self.map.insert(key, value);
        self.map.insert(key.to_string(), value);
    }

    #[napi(writable = false)]
    pub fn get(&mut self, key: String) -> napi::Result<Either<String, Undefined>> {
        if let Some(value) = self.map.get(&key) {
            return Ok(Either::A(value.to_string()));
        } else {
            return Ok(Either::B(() as Undefined));
        }
    }

    #[napi(writable = false)]
    pub fn delete(&mut self, key: String) {
        self.map.remove(&key);
    }

    #[napi(writable = false)]
    pub fn start_iter(&mut self) {
        self.iter = Some(self.map.clone().into_iter());
    }

    #[napi(writable = false)]
    pub fn next(&mut self, env: Env) -> Result<Either<Array, Undefined>> {
        if self.iter.is_none() {
            self.start_iter();
        }

        match self.iter.as_mut().unwrap().next() {
            Some((k, v)) => {
                let mut arr = env.create_array(2).unwrap();
                arr.set(0, k.to_string()).unwrap();
                arr.set(1, v.to_string()).unwrap();
                Ok(Either::A(arr))
            }
            None => Ok(Either::B(() as Undefined)),
        }
    }
}
