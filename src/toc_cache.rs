use crazyflie_lib::TocCache;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct FileTocCache {
    dir: PathBuf,
}

impl FileTocCache {
    pub fn new() -> Self {
        let dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("crazyflie-agent-cli")
            .join("toc-cache");
        fs::create_dir_all(&dir).ok();
        Self { dir }
    }
}

impl TocCache for FileTocCache {
    fn get_toc(&self, key: &[u8]) -> Option<String> {
        let path = self.dir.join(hex::encode(key));
        fs::read_to_string(path).ok()
    }

    fn store_toc(&self, key: &[u8], toc: &str) {
        let path = self.dir.join(hex::encode(key));
        fs::write(path, toc).ok();
    }
}
