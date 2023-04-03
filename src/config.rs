use anyhow::bail;
use netxclient::prelude::ServerOption;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: ServerOption,
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TlsConfig {
    pub ca: Option<PathBuf>,
    pub cert: PathBuf,
    pub key: PathBuf,
}

#[inline]
pub fn get_current_exec_path() -> std::io::Result<PathBuf> {
    Ok(match std::env::current_exe() {
        Ok(path) => {
            if let Some(current_exe_path) = path.parent() {
                current_exe_path.to_path_buf()
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "current_exe_path get error: is none",
                ));
            }
        }
        Err(err) => return Err(err),
    })
}

#[inline]
pub async fn load_config() -> anyhow::Result<Config> {
    let config_file = PathBuf::from("./config");
    if config_file.exists() {
        let config = tokio::fs::read_to_string(config_file).await?;
        Ok(toml::from_str(&config)?)
    } else {
        let mut current_exec_path = get_current_exec_path()?;
        current_exec_path.push("./config");
        if current_exec_path.exists() {
            Ok(toml::from_str(&std::fs::read_to_string(
                current_exec_path,
            )?)?)
        } else {
            bail!("not found config");
        }
    }
}
