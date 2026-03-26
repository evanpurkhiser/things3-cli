use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::wire::WireObject;
use anyhow::Result;
use std::collections::BTreeMap;

pub trait CloudWriter {
    fn commit(
        &mut self,
        changes: BTreeMap<String, WireObject>,
        ancestor_index: Option<i64>,
    ) -> Result<i64>;

    fn head_index(&self) -> i64;
}

pub struct LiveCloudWriter {
    client: ThingsCloudClient,
}

impl LiveCloudWriter {
    pub fn new() -> Result<Self> {
        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let _ = client.authenticate();
        Ok(Self { client })
    }
}

impl CloudWriter for LiveCloudWriter {
    fn commit(
        &mut self,
        changes: BTreeMap<String, WireObject>,
        ancestor_index: Option<i64>,
    ) -> Result<i64> {
        self.client.commit(changes, ancestor_index)
    }

    fn head_index(&self) -> i64 {
        self.client.head_index
    }
}
