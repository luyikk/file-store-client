use anyhow::{bail, Result};
use netxclient::prelude::*;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

/// client rpc interface
#[build(ClientController)]
pub trait IClientController {
    /// write buff to file by key
    #[tag(2001)]
    async fn write_file_by_key(&self, key: u64, offset: u64, data: Vec<u8>);
}

pub struct ClientController {
    fs: Arc<Actor<FileWriteService>>,
}

impl ClientController {
    pub fn new(fs: Arc<Actor<FileWriteService>>) -> Self {
        Self { fs }
    }
}

#[build_impl]
impl IClientController for ClientController {
    #[inline]
    async fn write_file_by_key(&self, key: u64, offset: u64, data: Vec<u8>) {
        if let Err(err) = self.fs.write_wfs_by_key(key, offset, data).await {
            log::error!("write_file_by_key err:{err}");
        }
    }
}

/// store fs and pipe
pub struct WriteHandle {
    fd: File,
    tx: tokio::sync::mpsc::Sender<u64>,
}

impl WriteHandle {
    pub fn new(fd: File, tx: tokio::sync::mpsc::Sender<u64>) -> Self {
        Self { fd, tx }
    }
}

pub struct FileWriteService {
    files: HashMap<u64, WriteHandle>,
}

impl FileWriteService {
    pub fn new() -> Arc<Actor<FileWriteService>> {
        Arc::new(Actor::new(FileWriteService {
            files: Default::default(),
        }))
    }
}

#[async_trait::async_trait]
pub trait IFileWS {
    /// create wfs
    async fn create_wfs(&self, key: u64, write_handle: WriteHandle);
    /// write wfs
    async fn write_wfs_by_key(&self, key: u64, offset: u64, data: Vec<u8>) -> Result<()>;
    /// close wfs
    async fn close_wfs(&self, key: u64) -> Result<()>;
}

#[async_trait::async_trait]
impl IFileWS for Actor<FileWriteService> {
    #[inline]
    async fn create_wfs(&self, key: u64, write_handle: WriteHandle) {
        self.inner_call(|inner| async move {
            inner.get_mut().files.insert(key, write_handle);
        })
        .await
    }

    #[inline]
    async fn write_wfs_by_key(&self, key: u64, offset: u64, data: Vec<u8>) -> Result<()> {
        self.inner_call(|inner| async move {
            if let Some(file) = inner.get_mut().files.get_mut(&key) {
                file.fd.seek(SeekFrom::Start(offset)).await?;
                file.fd.write_all(&data).await?;
                file.tx.send(data.len() as u64).await?;
                Ok(())
            } else {
                bail!("not found key:{}", key);
            }
        })
        .await
    }
    #[inline]
    async fn close_wfs(&self, key: u64) -> Result<()> {
        self.inner_call(|inner| async move {
            if let Some(mut wfs) = inner.get_mut().files.remove(&key) {
                wfs.fd.flush().await?;
            }
            Ok(())
        })
        .await
    }
}
