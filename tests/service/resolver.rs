use std::{
    net::{Ipv4Addr, Ipv6Addr},
    sync::Arc,
};

use async_trait::async_trait;
use trust_dns_resolver::{
    name_server::{GenericConnection, GenericConnectionProvider, TokioRuntime},
    AsyncResolver,
};

pub type Resolver = AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>;

pub type Resolvers = Vec<Arc<Resolver>>;

#[async_trait]
pub trait Lookup {
    async fn lookup_a(&self, record: String) -> Vec<Ipv4Addr>;
    async fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr>;
    async fn lookup_ptr(&self, record: String) -> Vec<String>;
}

#[async_trait]
impl Lookup for Resolver {
    async fn lookup_a(&self, record: String) -> Vec<Ipv4Addr> {
        self.ipv4_lookup(record)
            .await
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.data().unwrap().clone().into_a().unwrap())
            .collect()
    }

    async fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr> {
        self.ipv6_lookup(record)
            .await
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.data().unwrap().clone().into_aaaa().unwrap())
            .collect()
    }

    async fn lookup_ptr(&self, record: String) -> Vec<String> {
        self.reverse_lookup(record.parse().unwrap())
            .await
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.data().unwrap().clone().into_ptr().unwrap().to_string())
            .collect()
    }
}
