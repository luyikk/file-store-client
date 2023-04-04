mod clap_struct;
mod config;
mod interface_server;

use anyhow::{ensure, Context};
use clap::Parser;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use log::LevelFilter;
use netxclient::client::NetxClientArcDef;
use netxclient::prelude::*;
use rustls_pemfile::{certs, rsa_private_keys};
use std::fmt::Write;
use std::io::{BufReader, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_rustls::rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore, ServerName};

use crate::clap_struct::Opt;
use crate::config::{get_current_exec_path, load_config};
use crate::interface_server::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .filter_module("rustls", LevelFilter::Debug)
        .filter_module("mio", LevelFilter::Debug)
        .init();
    let opt = Opt::parse();

    if let Opt::Create = opt {
        let config = include_str!("../config.toml");
        std::fs::write("./config", config)?;
        return Ok(());
    }

    let config = load_config().await?;
    log::trace!("config:{:#?}", config);

    // create netx client
    let client = {
        if let Some(tls) = config.tls {
            let cert_path = if tls.cert.exists() {
                tls.cert
            } else {
                let mut current_exec_path = get_current_exec_path()?;
                current_exec_path.push(&tls.cert);
                ensure!(
                    current_exec_path.exists(),
                    "not found file:{:?}",
                    current_exec_path
                );
                current_exec_path
            };

            let key_path = if tls.key.exists() {
                tls.key
            } else {
                let mut current_exec_path = get_current_exec_path()?;
                current_exec_path.push(&tls.key);
                ensure!(
                    current_exec_path.exists(),
                    "not found file:{:?}",
                    current_exec_path
                );
                current_exec_path
            };

            let cert_file = &mut BufReader::new(std::fs::File::open(cert_path)?);
            let key_file = &mut BufReader::new(std::fs::File::open(key_path)?);

            let keys = PrivateKey(rsa_private_keys(key_file)?.remove(0));
            let cert_chain = certs(cert_file)
                .unwrap()
                .iter()
                .map(|c| Certificate(c.to_vec()))
                .collect::<Vec<_>>();

            if let Some(ca) = tls.ca {
                let ca_path = if ca.exists() {
                    ca
                } else {
                    let mut current_exec_path = get_current_exec_path()?;
                    current_exec_path.push(ca);
                    ensure!(
                        current_exec_path.exists(),
                        "not found file:{:?}",
                        current_exec_path
                    );
                    current_exec_path
                };

                let ca = &mut BufReader::new(std::fs::File::open(ca_path)?);
                let ca_certs = certs(ca)?;
                let mut server_auth_roots = RootCertStore::empty();
                server_auth_roots.add_parsable_certificates(&ca_certs);

                let tls_config = ClientConfig::builder()
                    .with_safe_defaults()
                    .with_root_certificates(server_auth_roots)
                    .with_single_cert(cert_chain, keys)
                    .expect("bad certificate/key");

                let connector = tokio_rustls::TlsConnector::from(Arc::new(tls_config));
                let domain = ServerName::try_from(config.server.addr.as_str())?;
                NetXClient::new_tls(
                    config.server,
                    DefaultSessionStore::default(),
                    domain,
                    connector,
                )
            } else {
                let tls_config = ClientConfig::builder()
                    .with_safe_defaults()
                    .with_custom_certificate_verifier(Arc::new(RustlsAcceptAnyCertVerifier))
                    .with_single_cert(cert_chain, keys)
                    .expect("bad certificate/key");
                let connector = tokio_rustls::TlsConnector::from(Arc::new(tls_config));
                let domain = ServerName::try_from(config.server.addr.split(':').next().unwrap())?;
                NetXClient::new_tls(
                    config.server,
                    DefaultSessionStore::default(),
                    domain,
                    connector,
                )
            }
        } else {
            NetXClient::new(config.server, DefaultSessionStore::default())
        }
    };

    if let Opt::Push { dir, file, r#async } = opt {
        push(client, dir, file, r#async).await?;
    }
    Ok(())
}

/// push file to server
#[inline]
async fn push(
    client: NetxClientArcDef,
    dir: Option<PathBuf>,
    file: PathBuf,
    r#async: bool,
) -> anyhow::Result<()> {
    ensure!(file.exists(), "not found file:{}", file.to_string_lossy());
    let file_name = file
        .file_name()
        .with_context(|| format!("file:{} not name", file.to_string_lossy()))?
        .to_string_lossy();

    let push_file_name = {
        if let Some(mut dir) = dir {
            dir.push(&*file_name);
            dir.to_string_lossy().replace('\\', "/").to_string()
        } else {
            file_name.to_string()
        }
    };

    let mut file = tokio::fs::File::open(file).await?;
    let size = file.metadata().await?.len();
    let start_hash = Instant::now();
    let hash = {
        let mut sha = blake3::Hasher::new();
        let mut data = vec![0; 1024 * 1024];
        while let Ok(len) = file.read(&mut data).await {
            if len > 0 {
                sha.update(&data[..len]);
            } else {
                break;
            }
        }
        hex::encode(sha.finalize().as_bytes())
    };
    log::debug!("hash computer time:{}", start_hash.elapsed().as_secs_f64());
    log::debug!(
        "start push file name:{} size:{}B hash:{}",
        push_file_name,
        size,
        hash
    );
    file.seek(SeekFrom::Start(0)).await?;

    let server = impl_struct!(client=>IFileStoreService);

    let key = server.push(&push_file_name, size, hash).await?;

    log::debug!("start write file:{push_file_name} key:{key}");

    let mut position = 0;
    let pb = ProgressBar::new(size);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));

    let mut buff = vec![0; 256 * 1024];
    while let Ok(len) = file.read(&mut buff).await {
        if len > 0 {
            if !r#async {
                server.write(key, &buff[..len]).await?;
            } else {
                server.write_offset(key, position, &buff[..len]).await;
            }
            position += len as u64;
            pb.set_position(position.min(size));
        } else {
            break;
        }
    }

    pb.set_position(size);

    if r#async {
        log::info!("wait 1 secs finish");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    server.push_finish(key).await?;
    Ok(())
}
