use netxclient::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
    /// 0=file 1=directory
    pub file_type: u8,
    pub name: String,
    pub size: u64,
    pub create_time: SystemTime,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub create_time: SystemTime,
    pub b3: Option<String>,
    pub sha256: Option<String>,
}

/// service interface
#[build]
pub trait IFileStoreService {
    /// push file
    ///
    /// filename:
    ///     file.xyz;
    ///     dict/file.xyz;
    ///
    /// size: file size u64
    ///
    /// hash: file BLAKE3
    ///
    /// return: file write key
    #[tag(1001)]
    async fn push(
        &self,
        filename: &str,
        size: u64,
        hash: String,
        overwrite: bool,
    ) -> anyhow::Result<u64>;
    /// write data to file
    /// key: file push key
    /// data: file data
    #[tag(1002)]
    async fn write(&self, key: u64, data: &[u8]) -> anyhow::Result<()>;
    /// write data to file
    /// key: file push key
    /// offset: file offset write position
    /// data: file data
    #[tag(1003)]
    async fn write_offset(&self, key: u64, offset: u64, data: &[u8]);
    /// finish write
    #[tag(1004)]
    async fn push_finish(&self, key: u64) -> anyhow::Result<()>;
    /// lock the filenames can be push
    #[tag(1005)]
    async fn lock(&self, filenames: &[String], overwrite: bool) -> anyhow::Result<(bool, String)>;
    /// check ready
    #[tag(1006)]
    async fn check_finish(&self, key: u64) -> anyhow::Result<bool>;
    /// show directory contents
    #[tag(1007)]
    async fn show_directory_contents(&self, path: PathBuf) -> anyhow::Result<Vec<Entry>>;
    /// get file info
    #[tag(1008)]
    async fn get_file_info(
        &self,
        path: PathBuf,
        blake3: bool,
        sha256: bool,
    ) -> anyhow::Result<FileInfo>;
}
