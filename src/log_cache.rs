use crate::client::ThingsCloudClient;
use crate::store::{fold_item, RawState};
use crate::wire::wire_object::WireItem;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
struct SyncSnapshot {
    history_key: Option<String>,
    head_index: i64,
}

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
    let mut safe_offset = byte_offset;

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }

        if !line.ends_with('\n') {
            break;
        }

        let stripped = line.trim();
        if stripped.is_empty() {
            safe_offset = reader.stream_position()?;
            continue;
        }
        let item: WireItem = serde_json::from_str(stripped)
            .with_context(|| format!("Corrupt log entry at {}", log_path.display()))?;
        fold_item(item, &mut state);
        new_lines += 1;
        safe_offset = reader.stream_position()?;
    }

    if new_lines > 0 {
        write_state_cache(cache_dir, &state, safe_offset)?;
    }

    Ok(state)
}

pub fn get_state_with_append_log(
    client: &mut ThingsCloudClient,
    cache_dir: PathBuf,
) -> Result<RawState> {
    let mut sync_client = client.clone();
    let sync_cache_dir = cache_dir.clone();

    let sync_worker = std::thread::spawn(move || -> Result<SyncSnapshot> {
        sync_append_log(&mut sync_client, &sync_cache_dir)?;
        Ok(SyncSnapshot {
            history_key: sync_client.history_key,
            head_index: sync_client.head_index,
        })
    });

    let _stale_state = fold_state_from_append_log(&cache_dir)?;

    let sync_snapshot = sync_worker
        .join()
        .map_err(|_| anyhow!("sync worker panicked"))??;

    client.history_key = sync_snapshot.history_key;
    client.head_index = sync_snapshot.head_index;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_state_ignores_trailing_partial_line() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let cache_dir = temp_dir.path();
        let log_path = cache_dir.join("things.log");

        let line_one = r#"{"3C6BBD49-8D11-4FFF-8B0E-B8F33FA9C00A":{"t":0,"e":"Settings5","p":{}}}"#;
        let line_two = r#"{"4C6BBD49-8D11-4FFF-8B0E-B8F33FA9C00B":{"t":0,"e":"Settings5","p":{}}}"#;
        let split_at = line_two.len() / 2;

        fs::write(
            &log_path,
            format!("{}\n{}", line_one, &line_two[..split_at]),
        )
        .expect("seed log");

        let first_state = fold_state_from_append_log(cache_dir).expect("first fold");
        assert_eq!(first_state.len(), 1);

        let (_, first_offset) = read_state_cache(cache_dir);
        assert_eq!(first_offset, (line_one.len() + 1) as u64);

        let mut fp = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .expect("open log for append");
        writeln!(fp, "{}", &line_two[split_at..]).expect("append line remainder");

        let second_state = fold_state_from_append_log(cache_dir).expect("second fold");
        assert_eq!(second_state.len(), 2);

        let expected_offset = fs::metadata(&log_path).expect("log metadata").len();
        let (_, second_offset) = read_state_cache(cache_dir);
        assert_eq!(second_offset, expected_offset);
    }
}
