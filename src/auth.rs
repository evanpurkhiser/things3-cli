use std::fs;

use anyhow::{Context, Result, anyhow};
use figment::{
    Figment,
    providers::{Env, Format, Json},
};
use serde::{Deserialize, Serialize};

use crate::dirs::auth_file_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthPayload {
    email: String,
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AuthConfig {
    email: Option<String>,
    password: Option<String>,
}

fn load_auth_config(path: &std::path::Path) -> Result<AuthConfig> {
    let mut figment = Figment::new();
    if path.exists() {
        figment = figment.merge(Json::file(path));
    }
    figment
        .merge(Env::prefixed("THINGS3_"))
        .extract()
        .with_context(|| format!("Failed reading auth config at {}", path.display()))
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

    let cfg = load_auth_config(&path)?;

    let Some(email) = cfg.email else {
        return Err(anyhow!(
            "Missing auth email. Set THINGS3_EMAIL or run `things3 set-auth` to create {}.",
            path.display()
        ));
    };

    let Some(password) = cfg.password else {
        return Err(anyhow!(
            "Missing auth password. Set THINGS3_PASSWORD or run `things3 set-auth` to update {}.",
            path.display()
        ));
    };

    validate_auth(&email, &password)
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
