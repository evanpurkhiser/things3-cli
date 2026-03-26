use crate::dirs::auth_file_path;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthPayload {
    email: String,
    password: String,
}

fn validate_auth(email: &str, password: &str) -> Result<(String, String)> {
    let email = email.trim().to_string();
    let password = password.to_string();

    if email.is_empty() {
        return Err(anyhow!("Missing auth email."));
    }
    if password.is_empty() {
        return Err(anyhow!("Missing auth password."));
    }

    Ok((email, password))
}

pub fn load_auth() -> Result<(String, String)> {
    let path = auth_file_path();
    if !path.exists() {
        return Err(anyhow!(
            "Auth not configured. Run `things3 set-auth` to create {}.",
            path.display()
        ));
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed reading auth config at {}", path.display()))?;
    let payload: AuthPayload = serde_json::from_str(&raw)
        .with_context(|| format!("Failed reading auth config at {}", path.display()))?;

    validate_auth(&payload.email, &payload.password)
}

pub fn write_auth(email: &str, password: &str) -> Result<std::path::PathBuf> {
    let (email, password) = validate_auth(email, password)?;
    let path = auth_file_path();
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Invalid auth file path"))?
        .to_path_buf();
    fs::create_dir_all(&parent).with_context(|| format!("Failed creating {}", parent.display()))?;

    let payload = AuthPayload { email, password };
    let serialized = serde_json::to_string(&payload)?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, serialized)
        .with_context(|| format!("Failed writing {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &path)
        .with_context(|| format!("Failed finalizing {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }

    Ok(path)
}
