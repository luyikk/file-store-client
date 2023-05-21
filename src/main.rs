mod clap_struct;
mod config;
mod controller;
mod interface_server;

use anyhow::{bail, ensure, Context};
use chrono::{DateTime, Local};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use log::LevelFilter;
use netxclient::client::NetxClientArcDef;
use netxclient::prelude::*;
use rustls_pemfile::{certs, rsa_private_keys};
use std::fmt::Write;
use std::io::{BufReader, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_rustls::rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore, ServerName};

use crate::clap_struct::{ImageArgs, ImageCommands, Opt};
use crate::config::{get_current_exec_path, load_config};
use crate::controller::{ClientController, FileWriteService, IFileWS, WriteHandle};
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

    match opt {
        Opt::Push {
            dir,
            file,
            r#async,
            block,
            overwrite,
        } => {
            push(client, dir, file, r#async, block, overwrite).await?;
        }
        Opt::Pull {
            file,
            save,
            r#async,
            block,
            overwrite,
        } => {
            pull_file(&client, file, save, r#async, block, overwrite).await?;
        }
        Opt::Image(ImageArgs {
            command:
                ImageCommands::Push {
                    dir,
                    path,
                    r#async,
                    block,
                    overwrite,
                },
        }) => {
            push_image(client, dir, path, r#async, block, overwrite).await?;
        }
        Opt::ShowDir { dir } => {
            show_dir(client, dir).await?;
        }
        Opt::Info { file } => {
            show_file_info(client, file).await?;
        }
        _ => {}
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
    block: usize,
    overwrite: bool,
) -> anyhow::Result<()> {
    ensure!(file.is_file(), "path:{} not file", file.display());
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
    log::trace!("hash computer time:{}", start_hash.elapsed().as_secs_f64());
    log::trace!(
        "start push file name:{} size:{}B hash:{}",
        push_file_name,
        size,
        hash
    );
    file.seek(SeekFrom::Start(0)).await?;

    let server = impl_struct!(client=>IFileStoreService);
    let key = server.push(&push_file_name, size, hash, overwrite).await?;
    log::debug!("start write file:{push_file_name} key:{key}");
    let mut position = 0;
    let pb = ProgressBar::new(size);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));

    let mut buff = vec![0; block];
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

    pb.finish_with_message("upload success");

    if r#async {
        let mut retry_count = 0;
        while !server.check_finish(key).await? && retry_count < 20 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            retry_count += 1;
        }
    }

    server.push_finish(key).await?;
    Ok(())
}

/// push image path
#[inline]
async fn push_image(
    client: NetxClientArcDef,
    dir: Option<PathBuf>,
    path: PathBuf,
    r#async: bool,
    block: usize,
    overwrite: bool,
) -> anyhow::Result<()> {
    ensure!(path.is_dir(), "path:{} not dir", path.display());
    ensure!(path.exists(), "not found path:{}", path.display());

    #[inline]
    fn visit_dirs(dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, files)?;
                } else {
                    files.push(entry.path());
                }
            }
        }
        Ok(())
    }

    let mut files = vec![];
    visit_dirs(&path, &mut files)?;

    ensure!(
        !files.is_empty(),
        "path:{} is empty directory",
        path.display()
    );

    let relative_files = files
        .iter()
        .map(|file| {
            let parent = if let Some(base) = path.parent() {
                file.strip_prefix(base).unwrap().parent().unwrap()
            } else {
                file.parent().unwrap()
            };

            if let Some(ref dir) = dir {
                PathBuf::from(dir.join(parent).to_string_lossy().replace('\\', "/"))
            } else {
                PathBuf::from(parent.to_string_lossy().replace('\\', "/"))
            }
        })
        .collect::<Vec<_>>();

    let check_files = relative_files
        .iter()
        .zip(files.iter())
        .map(|(base, file)| {
            base.join(file.file_name().unwrap())
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();

    let server = impl_struct!(client=>IFileStoreService);

    log::debug!("start check path:{}", path.display());
    let (success, msg) = server.lock(&check_files, overwrite).await?;

    if success {
        /// push file
        #[inline]
        async fn push_file(
            client: NetxClientArcDef,
            progress: &ProgressBar,
            push_file_name: String,
            file: PathBuf,
            r#async: bool,
            block: usize,
            overwrite: bool,
        ) -> anyhow::Result<()> {
            ensure!(file.is_file(), "path:{} not file", file.display());
            ensure!(file.exists(), "not found file:{}", file.to_string_lossy());
            let mut file = tokio::fs::File::open(file).await?;
            let size = file.metadata().await?.len();
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

            file.seek(SeekFrom::Start(0)).await?;

            let server = impl_struct!(client=>IFileStoreService);
            let key = server.push(&push_file_name, size, hash, overwrite).await?;

            let mut position = 0;
            progress.set_length(size);
            progress.reset();

            let mut buff = vec![0; block];
            while let Ok(len) = file.read(&mut buff).await {
                if len > 0 {
                    if !r#async {
                        server.write(key, &buff[..len]).await?;
                    } else {
                        server.write_offset(key, position, &buff[..len]).await;
                    }
                    position += len as u64;
                    progress.set_position(position.min(size));
                } else {
                    break;
                }
            }

            progress.finish();
            if r#async {
                let mut retry_count = 0;
                while !server.check_finish(key).await? && retry_count < 20 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    retry_count += 1;
                }
            }
            server.push_finish(key).await?;
            Ok(())
        }

        let multi_progress = MultiProgress::new();
        let file_pb = multi_progress.add(ProgressBar::new(files.len() as u64));
        file_pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("##-"),
        );

        let write_pb = multi_progress.add(ProgressBar::new(0));
        write_pb.set_style(ProgressStyle::with_template("{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));

        for (file, push_file_name) in files.into_iter().zip(check_files.into_iter()) {
            file_pb.set_message(format!("start push file:{}", push_file_name));
            push_file(
                client.clone(),
                &write_pb,
                push_file_name,
                file,
                r#async,
                block,
                overwrite,
            )
            .await?;
            file_pb.inc(1);
        }
        file_pb.finish_with_message("image push finish");
    } else {
        log::error!("check path:{} error:{}", path.display(), msg);
    }

    Ok(())
}

/// show directory contexts
#[inline]
async fn show_dir(client: NetxClientArcDef, dir: PathBuf) -> anyhow::Result<()> {
    use console::style;
    use humansize::{format_size, WINDOWS};
    let server = impl_struct!(client=>IFileStoreService);
    let mut files = server.show_directory_contents(dir).await?;
    files.sort_by(|a, b| b.file_type.cmp(&a.file_type));
    for entry in files {
        if entry.file_type == 1 {
            let datetime = DateTime::<Local>::from(entry.create_time);
            println!(
                "{:10}         {}      {}/",
                style(format_size(0u32, WINDOWS)).yellow().bold(),
                style(datetime.format("%d/%m/%Y %T")).green().bold(),
                style(entry.name).blue().bold()
            );
        } else {
            let datetime = DateTime::<Local>::from(entry.create_time);
            println!(
                "{:10}         {}      {}",
                style(format_size(entry.size, WINDOWS)).yellow().bold(),
                style(datetime.format("%d/%m/%Y %T")).green().bold(),
                style(entry.name).cyan().bold()
            );
        }
    }

    Ok(())
}

/// show file info
#[inline]
async fn show_file_info(client: NetxClientArcDef, file: PathBuf) -> anyhow::Result<()> {
    use console::style;
    use humansize::{format_size, WINDOWS};
    let server = impl_struct!(client=>IFileStoreService);
    let info = server.get_file_info(&file, true, true).await?;
    println!(
        "file name: {}\nsize: {} Byte ({})\nblake3: {}\nsha256: {}\ncreate time: {}\ncan modify: {}",
        style(info.name).cyan().bold(),
        style(info.size).yellow().bold(),
        style(format_size(info.size, WINDOWS)).yellow(),
        style(info.b3.as_ref().map_or("none",|x|x.as_str())).blue().bold(),
        style(info.sha256.as_ref().map_or("none",|x|x.as_str())).red().bold(),
        style(DateTime::<Local>::from(info.create_time).format("%d/%m/%Y %T"))
            .green()
            .bold(),
        style(info.can_modify)
            .white()
            .bold()
    );
    Ok(())
}

/// sync pull file
#[inline]
async fn pull_file(
    client: &NetxClientArcDef,
    file: PathBuf,
    save: Option<PathBuf>,
    r#async: bool,
    block: usize,
    overwrite: bool,
) -> anyhow::Result<()> {
    let server = impl_struct!(client=>IFileStoreService);
    let info = server.get_file_info(&file, true, false).await?;
    ensure!(
        info.b3.is_some(),
        "currently unable to pull file:{}",
        file.display()
    );

    let save_path = {
        if let Some(save) = save {
            if save.is_dir() {
                save.join(&info.name)
            } else {
                save
            }
        } else {
            PathBuf::from(&info.name)
        }
    };

    if save_path.exists() {
        if !overwrite {
            bail!("file:{} already exists", save_path.display())
        } else {
            std::fs::remove_file(&save_path)?;
        }
    }

    log::info!("start pull file:{}", save_path.display());
    let key = server.create_pull(&file).await?;

    let mut fd = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&save_path)
        .await?;

    let size = info.size;
    log::debug!("file size:{}", size);
    let pb = ProgressBar::new(size);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .progress_chars("#>-"));

    if r#async {
        let wfs = FileWriteService::new();
        let controller = ClientController::new(wfs.clone());
        client.init(controller).await?;

        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
        wfs.create_wfs(key, WriteHandle::new(fd, tx)).await;

        server.async_read(key, block).await;

        let mut offset: u64 = 0;
        while let Some(r_size) = rx.recv().await {
            offset += r_size;
            pb.set_position(offset.min(size));
            if offset >= size {
                break;
            }
        }
        wfs.close_wfs(key).await?;
    } else {
        let mut offset = 0;
        while let Ok(data) = server.read(key, offset, block).await {
            if !data.is_empty() {
                offset += data.len() as u64;
                fd.write_all(&data).await?;
                pb.set_position(offset.min(size));
            } else {
                break;
            }
        }
        fd.flush().await?;
        drop(fd);
    }

    pb.finish_with_message("downloaded success");
    server.finish_read_key(key).await;

    let b3 = {
        let mut sha = blake3::Hasher::new();
        let mut data = vec![0; 512 * 1024];
        let mut file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&save_path)
            .await?;
        while let Ok(len) = file.read(&mut data).await {
            if len > 0 {
                sha.update(&data[..len]);
            } else {
                break;
            }
        }
        hex::encode(sha.finalize().as_bytes())
    };

    if &b3 != info.b3.as_ref().unwrap() {
        std::fs::remove_file(save_path)?;
        bail!(
            "file read hash error remote b3:{} local b3:{}",
            info.b3.unwrap(),
            b3
        );
    } else {
        log::info!("pull file:{} success", save_path.display());
    }

    Ok(())
}
