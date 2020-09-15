use std::cmp::{Ordering, PartialEq, PartialOrd};
use std::convert::TryFrom;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone)]
pub struct MetaFile {
    pub name: PathBuf,
    pub dir: bool,
    pub size: u64,
    pub mtime: u64,
}

impl PartialEq for MetaFile {
    fn eq(&self, other: &MetaFile) -> bool {
        self.name.eq(&other.name)
    }
}
impl PartialOrd for MetaFile {
    fn partial_cmp(&self, other: &MetaFile) -> Option<Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl TryFrom<&(PathBuf, ssh2::FileStat)> for MetaFile {
    type Error = String;
    fn try_from((n, f): &(PathBuf, ssh2::FileStat)) -> Result<Self, Self::Error> {
        let name = match n.file_name() {
            None => return Err(format!("No file name for {:?}", n)),
            Some(n) => PathBuf::from(n),
        };
        let mut m = MetaFile {
            name: name,
            dir: f.is_dir(),
            size: 0,
            mtime: f.mtime.unwrap_or(0),
        };
        if !m.dir {
            m.size = f.size.ok_or(format!("File {:?} has no size", n))?
        }
        Ok(m)
    }
}
impl TryFrom<DirEntry> for MetaFile {
    type Error = String;
    fn try_from(f: DirEntry) -> Result<Self, Self::Error> {
        let name = PathBuf::from(f.file_name());
        let info = f
            .metadata()
            .map_err(|err| format!("Get Metadato of {:?} fail: {}", name, err))?;

        Ok(MetaFile {
            mtime: match info
                .modified()
                .map_err(|err| format!("Get modified information about {:?} {}", name, err))?
                .duration_since(SystemTime::UNIX_EPOCH)
            {
                Ok(d) => d.as_secs(),
                Err(_) => 0,
            },
            name: name,
            dir: info.is_dir(),
            size: info.len(),
        })
    }
}
