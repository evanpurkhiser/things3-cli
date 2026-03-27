use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::wire::wire_object::WireObject;
use anyhow::Result;
use std::collections::BTreeMap;
use tracing::{debug, error};

pub trait CloudWriter {
    fn commit(
        &mut self,
        changes: BTreeMap<String, WireObject>,
        ancestor_index: Option<i64>,
    ) -> Result<i64>;

    fn head_index(&self) -> i64;
}

pub struct LoggingCloudWriter {
    inner: Box<dyn CloudWriter>,
}

impl LoggingCloudWriter {
    pub fn new(inner: Box<dyn CloudWriter>) -> Self {
        Self { inner }
    }
}

impl CloudWriter for LoggingCloudWriter {
    fn commit(
        &mut self,
        changes: BTreeMap<String, WireObject>,
        ancestor_index: Option<i64>,
    ) -> Result<i64> {
        let uuids = changes.keys().cloned().collect::<Vec<_>>();
        debug!(
            event = "cloud.commit.request",
            ancestor_index,
            change_count = uuids.len(),
            uuids = ?uuids,
            "cloud commit request"
        );

        match self.inner.commit(changes, ancestor_index) {
            Ok(head_index) => {
                debug!(
                    event = "cloud.commit.success",
                    ancestor_index,
                    change_count = uuids.len(),
                    uuids = ?uuids,
                    head_index,
                    "cloud commit succeeded"
                );
                Ok(head_index)
            }
            Err(err) => {
                error!(
                    event = "cloud.commit.error",
                    ancestor_index,
                    change_count = uuids.len(),
                    uuids = ?uuids,
                    error = %err,
                    "cloud commit failed"
                );
                Err(err)
            }
        }
    }

    fn head_index(&self) -> i64 {
        self.inner.head_index()
    }
}

pub struct LiveCloudWriter {
    client: ThingsCloudClient,
}

#[derive(Default)]
pub struct DryRunCloudWriter {
    head_index: i64,
}

impl DryRunCloudWriter {
    pub fn new() -> Self {
        Self::default()
    }
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

impl CloudWriter for DryRunCloudWriter {
    fn commit(
        &mut self,
        _changes: BTreeMap<String, WireObject>,
        _ancestor_index: Option<i64>,
    ) -> Result<i64> {
        self.head_index += 1;
        Ok(self.head_index)
    }

    fn head_index(&self) -> i64 {
        self.head_index
    }
}
