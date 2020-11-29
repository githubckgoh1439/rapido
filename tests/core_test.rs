use abci::*;
use anyhow::bail;
use borsh::{BorshDeserialize, BorshSerialize};
use exonum_crypto::Hash;
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut},
    BinaryValue, ObjectHash, Snapshot, TemporaryDB,
};
use std::sync::Arc;
use std::{borrow::Cow, convert::AsRef};

#[macro_use]
extern crate rapido;

use rapido::{AppBuilder, AppModule, Context, SignedTransaction};
#[derive(Debug, Clone, PartialEq, BorshSerialize, BorshDeserialize, Default)]
pub struct Count(u16);
impl Count {
    pub fn inc(&mut self) {
        self.0 += 1;
    }
}

impl_store_values!(Count);

#[derive(Debug)]
pub(crate) struct CountStore<T: Access> {
    access: T,
}
impl<T: Access> CountStore<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    pub fn get_count(&self) -> Count {
        self.access
            .get_proof_entry("counter")
            .get()
            .unwrap_or_default()
    }
}
impl<T: Access> CountStore<T>
where
    T::Base: RawAccessMut,
{
    pub fn increment(&mut self) {
        let mut count = self.get_count();
        count.inc();
        self.access.get_proof_entry("counter").set(count);
    }
}

pub struct CounterExample;
impl AppModule for CounterExample {
    fn name(&self) -> &'static str {
        "counter"
    }

    fn handle_tx(&self, ctx: &Context) -> Result<(), anyhow::Error> {
        let mut store = CountStore::new(ctx.fork);
        store.increment();
        // Emit an event from this call
        ctx.dispatch_event("count", &[("added", "one")]);
        Ok(())
    }

    fn handle_query(
        &self,
        path: &str,
        _key: Vec<u8>,
        snapshot: &Box<dyn Snapshot>,
    ) -> Result<Vec<u8>, anyhow::Error> {
        match path {
            "/" => query_count(snapshot),
            "/random" => query_random(),
            _ => bail!(""),
        }
    }
}

//
// declare_storage! {
//   Hello: String => Count);
//   Another: u32 => MyValue;
//}

//declare_store!(MyCount<T>, String, Count);

// Queries
fn query_count(snapshot: &Box<dyn Snapshot>) -> Result<Vec<u8>, anyhow::Error> {
    let store = CountStore::new(snapshot);
    let cnt = store.get_count();
    Ok(cnt.to_bytes())
}

fn query_random() -> Result<Vec<u8>, anyhow::Error> {
    Ok(vec![1])
}

// Helpers
fn generate_tx() -> SignedTransaction {
    SignedTransaction::new([0u8; 10].to_vec(), "counter", 0u8, vec![0], 0u64)
}

fn send_tx() -> RequestDeliverTx {
    let tx = generate_tx().encode();
    let mut req = RequestDeliverTx::new();
    req.set_tx(tx.clone());
    req
}

#[test]
fn test_basics() {
    let db = Arc::new(TemporaryDB::new());
    let mut node = AppBuilder::new(db)
        .add_service(Box::new(CounterExample {}))
        .finish();

    node.init_chain(&RequestInitChain::new());
    let resp = node.commit(&RequestCommit::new());
    assert!(resp.data.len() > 0);

    let resp = node.deliver_tx(&send_tx());
    assert_eq!(0u32, resp.code);
    println!("{:?}", resp.events);
    assert_eq!(1, resp.events.len());
    let c1 = node.commit(&RequestCommit::new());
    assert_ne!(resp.data, c1.data);

    {
        let mut query = RequestQuery::new();
        query.path = "counter".into();
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        let c = Count::try_from_slice(&resp.value[..]).unwrap();
        assert_eq!(1, c.0);
    }

    {
        let mut query = RequestQuery::new();
        query.path = "counter/random".into();
        let resp = node.query(&query);
        assert_eq!(0u32, resp.code);
        assert_eq!(vec![1], resp.value);
    }

    {
        let mut query = RequestQuery::new();
        query.path = "counter/fail".into();
        let resp = node.query(&query);
        assert_eq!(1u32, resp.code);
    }
}
