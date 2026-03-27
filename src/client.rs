use crate::store::{fold_item, RawState};
use crate::wire::task::{TaskPatch, TaskStatus};
use crate::wire::wire_object::WireItem;
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use urlencoding::encode;

const BASE_URL: &str = "https://cloud.culturedcode.com/version/1";
const USER_AGENT: &str = "ThingsMac/32209501";
const CLIENT_INFO: &str = "eyJkbSI6Ik1hYzE0LDIiLCJsciI6IlVTIiwibmYiOnRydWUsIm5rIjp0cnVlLCJubiI6IlRoaW5nc01hYyIsIm52IjoiMzIyMDk1MDEiLCJvbiI6Im1hY09TIiwib3YiOiIyNi4zLjAiLCJwbCI6ImVuLVVTIiwidWwiOiJlbi1MYXRuLVVTIn0=";
const APP_ID: &str = "com.culturedcode.ThingsMac";
const SCHEMA: &str = "301";
const WRITE_PUSH_PRIORITY: &str = "10";

fn app_instance_id() -> String {
    std::env::var("THINGS_APP_INSTANCE_ID").unwrap_or_else(|_| "things-cli".to_string())
}

fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub(crate) fn now_timestamp() -> f64 {
    now_ts()
}

#[derive(Debug, Clone)]
pub struct ThingsCloudClient {
    pub email: String,
    pub password: String,
    pub history_key: Option<String>,
    pub head_index: i64,
    http: Client,
}

impl ThingsCloudClient {
    pub fn new(email: String, password: String) -> Result<Self> {
        let http = Client::builder().build()?;
        Ok(Self {
            email,
            password,
            history_key: None,
            head_index: 0,
            http,
        })
    }

    fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<Value>,
        extra_headers: &[(&str, String)],
    ) -> Result<Value> {
        let mut req = self
            .http
            .request(method, url)
            .header("Accept", "application/json")
            .header("Accept-Charset", "UTF-8")
            .header("User-Agent", USER_AGENT)
            .header("things-client-info", CLIENT_INFO)
            .header("App-Id", APP_ID)
            .header("Schema", SCHEMA)
            .header("App-Instance-Id", app_instance_id());

        for (k, v) in extra_headers {
            req = req.header(*k, v);
        }

        if let Some(payload) = body {
            req = req
                .header("Content-Type", "application/json; charset=UTF-8")
                .header("Content-Encoding", "UTF-8")
                .json(&payload);
        }

        let resp = req
            .send()
            .with_context(|| format!("request failed: {url}"))?;
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("HTTP {} for {}: {}", status.as_u16(), url, text));
        }
        if text.trim().is_empty() {
            return Ok(json!({}));
        }
        serde_json::from_str(&text).with_context(|| format!("invalid json from {url}"))
    }

    pub fn authenticate(&mut self) -> Result<String> {
        let url = format!("{BASE_URL}/account/{}", encode(&self.email));
        let result = self.request(
            reqwest::Method::GET,
            &url,
            None,
            &[(
                "Authorization",
                format!("Password {}", encode(&self.password)),
            )],
        )?;
        let key = result
            .get("history-key")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("missing history-key in auth response"))?
            .to_string();
        self.history_key = Some(key.clone());
        Ok(key)
    }

    pub fn get_items_page(&self, start_index: i64) -> Result<Value> {
        let history_key = self
            .history_key
            .as_ref()
            .ok_or_else(|| anyhow!("Must authenticate first"))?;
        let url = format!("{BASE_URL}/history/{history_key}/items?start-index={start_index}");
        self.request(reqwest::Method::GET, &url, None, &[])
    }

    pub fn get_all_items(&mut self) -> Result<RawState> {
        if self.history_key.is_none() {
            let _ = self.authenticate()?;
        }

        let mut state = RawState::new();
        let mut start_index = 0i64;

        loop {
            let page = self.get_items_page(start_index)?;
            let items = page
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let item_count = items.len();
            self.head_index = page
                .get("current-item-index")
                .and_then(Value::as_i64)
                .unwrap_or(self.head_index);

            for item in items {
                let wire: WireItem = serde_json::from_value(item)?;
                fold_item(wire, &mut state);
            }

            let end = page
                .get("end-total-content-size")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let latest = page
                .get("latest-total-content-size")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            if end >= latest {
                break;
            }
            start_index += item_count as i64;
        }

        Ok(state)
    }

    pub fn commit(
        &mut self,
        changes: BTreeMap<String, WireObject>,
        ancestor_index: Option<i64>,
    ) -> Result<i64> {
        let history_key = self
            .history_key
            .as_ref()
            .ok_or_else(|| anyhow!("Must authenticate first"))?;
        let idx = ancestor_index.unwrap_or(self.head_index);
        let url = format!("{BASE_URL}/history/{history_key}/commit?ancestor-index={idx}&_cnt=1");

        let mut payload = BTreeMap::new();
        for (uuid, obj) in changes {
            payload.insert(uuid, obj);
        }

        let result = self.request(
            reqwest::Method::POST,
            &url,
            Some(serde_json::to_value(payload)?),
            &[("Push-Priority", WRITE_PUSH_PRIORITY.to_string())],
        )?;

        let new_index = result
            .get("server-head-index")
            .and_then(Value::as_i64)
            .unwrap_or(idx);
        self.head_index = new_index;
        Ok(new_index)
    }

    pub fn set_task_status(
        &mut self,
        task_uuid: &str,
        status: i32,
        entity: Option<String>,
        stop_date: Option<f64>,
    ) -> Result<i64> {
        let entity_type = entity
            .map(|e| EntityType::from(e))
            .unwrap_or(EntityType::Task6);
        let mut changes = BTreeMap::new();
        changes.insert(
            task_uuid.to_string(),
            WireObject::update(
                entity_type,
                TaskPatch {
                    status: Some(TaskStatus::from(status)),
                    stop_date: Some(stop_date),
                    modification_date: Some(now_ts()),
                    ..Default::default()
                },
            ),
        );
        self.commit(changes, None)
    }
}
