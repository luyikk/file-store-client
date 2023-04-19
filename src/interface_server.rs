use netxclient::prelude::*;

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
}
