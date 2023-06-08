#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

mod mods {
    mod dual_kv;
    mod kv_map;
}
