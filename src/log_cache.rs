use crate::client::ThingsCloudClient;
use crate::store::{RawState, fold_item};
use crate::wire::wire_object::WireItem;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CursorData {
    next_start_index: i64,
    history_key: String,
    #[serde(default)]
    head_index: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StateCacheData {
    #[serde(default)]
    version: u8,
    log_offset: u64,
    state: RawState,
}

const STATE_CACHE_VERSION: u8 = 2;

fn read_cursor(path: &Path) -> CursorData {
    if !path.exists() {
        return CursorData::default();
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return CursorData::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_cursor(
    path: &Path,
    next_start_index: i64,
    history_key: &str,
    head_index: i64,
) -> Result<()> {
    let payload = serde_json::to_string(&serde_json::json!({
        "next_start_index": next_start_index,
        "history_key": history_key,
        "head_index": head_index,
        "updated_at": crate::client::now_timestamp(),
    }))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, payload)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn sync_append_log(client: &mut ThingsCloudClient, cache_dir: &Path) -> Result<()> {
    fs::create_dir_all(cache_dir)?;
    let log_path = cache_dir.join("things.log");
    let cursor_path = cache_dir.join("cursor.json");

    let cursor = read_cursor(&cursor_path);
    let mut start_index = cursor.next_start_index;

    if client.history_key.is_none() {
        if !cursor.history_key.is_empty() {
            client.history_key = Some(cursor.history_key.clone());
        } else {
            let _ = client.authenticate()?;
        }
    }

    let mut fp = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open {}", log_path.display()))?;

    loop {
        let page = match client.get_items_page(start_index) {
            Ok(v) => v,
            Err(_) => {
                let _ = client.authenticate()?;
                client.get_items_page(start_index)?
            }
        };

        let items = page
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let end = page
            .get("end-total-content-size")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let latest = page
            .get("latest-total-content-size")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        client.head_index = page
            .get("current-item-index")
            .and_then(Value::as_i64)
            .unwrap_or(client.head_index);

        for item in &items {
            writeln!(fp, "{}", serde_json::to_string(item)?)?;
        }

        if !items.is_empty() {
            fp.flush()?;
            start_index += items.len() as i64;
            write_cursor(
                &cursor_path,
                start_index,
                client.history_key.as_deref().unwrap_or_default(),
                client.head_index,
            )?;
        }

        if items.is_empty() || end >= latest {
            break;
        }
    }

    let current_history_key = client.history_key.clone().unwrap_or_default();
    if current_history_key != cursor.history_key || client.head_index != cursor.head_index {
        write_cursor(
            &cursor_path,
            start_index,
            &current_history_key,
            client.head_index,
        )?;
    }

    Ok(())
}

fn read_state_cache(cache_dir: &Path) -> (RawState, u64) {
    let path = cache_dir.join("state_cache.json");
    if !path.exists() {
        return (RawState::new(), 0);
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return (RawState::new(), 0);
    };
    let Ok(cache) = serde_json::from_str::<StateCacheData>(&raw) else {
        return (RawState::new(), 0);
    };

    if cache.version != STATE_CACHE_VERSION {
        return (RawState::new(), 0);
    }

    (cache.state, cache.log_offset)
}

fn write_state_cache(cache_dir: &Path, state: &RawState, log_offset: u64) -> Result<()> {
    let path = cache_dir.join("state_cache.json");
    let payload = serde_json::to_string(&StateCacheData {
        version: STATE_CACHE_VERSION,
        log_offset,
        state: state.clone(),
    })?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, payload)?;
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn fold_state_from_append_log(cache_dir: &Path) -> Result<RawState> {
    let log_path = cache_dir.join("things.log");
    if !log_path.exists() {
        return Ok(RawState::new());
    }

    let (mut state, byte_offset) = read_state_cache(cache_dir);
    let mut new_lines = 0u64;

    let mut file =
        File::open(&log_path).with_context(|| format!("failed to open {}", log_path.display()))?;
    file.seek(SeekFrom::Start(byte_offset))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let item: WireItem = serde_json::from_str(stripped)
            .with_context(|| format!("Corrupt log entry at {}", log_path.display()))?;
        fold_item(item, &mut state);
        new_lines += 1;
    }

    let end_offset = reader.stream_position()?;
    if new_lines > 0 {
        write_state_cache(cache_dir, &state, end_offset)?;
    }

    Ok(state)
}

pub fn get_state_with_append_log(
    client: &mut ThingsCloudClient,
    cache_dir: PathBuf,
) -> Result<RawState> {
    sync_append_log(client, &cache_dir)?;
    fold_state_from_append_log(&cache_dir)
}

pub fn fold_state_from_append_log_or_empty(cache_dir: &Path) -> RawState {
    fold_state_from_append_log(cache_dir).unwrap_or_default()
}

pub fn read_cached_head_index(cache_dir: &Path) -> i64 {
    read_cursor(&cache_dir.join("cursor.json")).head_index
}

pub fn sync_append_log_or_err(client: &mut ThingsCloudClient, cache_dir: &Path) -> Result<()> {
    sync_append_log(client, cache_dir).map_err(|e| anyhow!(e.to_string()))
}
