#![allow(dead_code)]

pub mod central_api {
    include!(concat!(env!("OUT_DIR"), "/central.rs"));
}

pub mod service_api {
    include!(concat!(env!("OUT_DIR"), "/service.rs"));
}
