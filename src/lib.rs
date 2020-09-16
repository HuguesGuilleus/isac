#[macro_use]
extern crate lazy_static;

use separator::Separatable;
use ssh2::{Session, Sftp};
use std::fs::{create_dir_all, read_dir, remove_dir_all, File};
use std::path::PathBuf;

mod addr;
pub use addr::{addr_from_reader, Addr};
mod linkvec;
use linkvec::{linkvec, Couple};

mod metafile;
use metafile::MetaFile;

struct Assets {
    a: Addr,
    sftp: Sftp,
    ansi: bool,
}
impl Assets {
    fn new(a: Addr, ansi: bool) -> Result<Assets, String> {
        Ok(Assets {
            ansi: ansi,
            sftp: Assets::connect(&a)?,
            a: a,
        })
    }
    fn connect(a: &Addr) -> Result<Sftp, String> {
        let mut s = Session::new()
            .map_err(|err| format!("The creation of a new SSH session fail: {}", err))?;
        s.set_compress(true);
        s.set_tcp_stream(a.connect()?);
        s.handshake()
            .map_err(|err| format!("SSH Handshake fail for {}: {}", a, err))?;
        s.userauth_agent(&a.user)
            .map_err(|err| format!("Authentification fail for {}: {}", a, err))?;

        s.sftp()
            .map_err(|err| format!("Open SFTP fail for {}: {}", a, err))
    }
    fn log(&self, op: &str, path: &PathBuf, size: Option<u64>) {
        let p = path.to_str().unwrap_or("");

        if self.ansi {
            print!(
                "{:>12}: \x1b[1;34m{}@{}\x1b[36m{}\x1b[0m",
                op, self.a.user, self.a.host, p
            );
        } else {
            print!("{:>12}: <{}@{}> {}", op, self.a.user, self.a.host, p);
        }

        match size {
            Some(size) => print!(" ({} o)", size.separated_string()),
            None => {}
        }

        println!("");
    }
}

/* DOWNLOAD */

pub fn donwload(a: Addr, ansi: bool) -> Result<(), String> {
    create_dir_all(&a.digest)
        .map_err(|err| format!("Create {:?} directory fail: {}", a.digest, err))?;

    let assets = Assets::new(a, ansi)?;

    download_dir(
        &assets,
        &PathBuf::from(&assets.a.root),
        &PathBuf::from(&assets.a.digest),
    )
}

fn download_dir(a: &Assets, remote_dir: &PathBuf, local_dir: &PathBuf) -> Result<(), String> {
    use std::convert::TryFrom;

    a.log("index", remote_dir, None);

    let remote_list: Vec<MetaFile> = a
        .sftp
        .readdir(remote_dir)
        .map_err(|err| format!("Index remote directory {:?} fail: {}", remote_dir, err))?
        .iter()
        .filter_map(|f| match MetaFile::try_from(f) {
            Ok(meta) => Some(meta),
            Err(err) => {
                eprintln!("ERROR in remote directory {:?}: {} ", remote_dir, err);
                None
            }
        })
        .collect();

    let local_list: Vec<MetaFile> = read_dir(local_dir)
        .map_err(|err| format!("Index local directory {:?} fail: {}", local_dir, err))?
        .filter_map(|r| match r {
            Ok(f) => Some(f),
            Err(err) => {
                eprintln!("ERROR in local directory {:?}: {} ", local_dir, err);
                None
            }
        })
        .filter_map(|f| match MetaFile::try_from(f) {
            Ok(meta) => Some(meta),
            Err(err) => {
                eprintln!("ERROR in remote directory {:?}: {} ", remote_dir, err);
                None
            }
        })
        .collect();

    linkvec(remote_list, local_list)
        .iter()
        .map(|couple| download_couple(a, couple, &remote_dir, &local_dir))
        .filter_map(|r| r.err())
        .for_each(|err| eprintln!("\x1b[K\x1b[1;41m ERROR \x1b[0m {}", err));

    Ok(())
}

fn download_couple(
    a: &Assets,
    couple: &Couple<MetaFile>,
    remote_dir: &PathBuf,
    local_dir: &PathBuf,
) -> Result<(), String> {
    match couple {
        (Some(remote), Some(local)) => {
            let remote_path = remote_dir.join(&remote.name);
            let local_path = local_dir.join(&local.name);
            match (remote.dir, local.dir) {
                (true, true) => download_dir(a, &remote_path, &local_path),
                (true, false) => {
                    a.log("rm", &local_path, None);
                    std::fs::remove_file(&local_path)
                        .map_err(|err| format!("Remove {:?} fail: {}", local_path, err))?;
                    download_couple(a, &(Some(remote.clone()), None), remote_dir, local_dir)
                }
                (false, true) => {
                    a.log("rmdir", &local_dir, None);
                    remove_dir_all(&local_path)
                        .map_err(|err| format!("Remove dir {:?} fail {}", local_path, err))?;
                    a.log("download", &remote_path, Some(remote.size));
                    download_file(a, &remote_path, &local_path)
                }
                (false, false) => {
                    if remote.mtime < local.mtime {
                        return Ok(());
                    }
                    a.log("download", &remote_path, Some(remote.size));
                    download_file(a, &remote_path, &local_path)
                }
            }
        }
        (Some(f), None) => {
            let remote_path = remote_dir.join(&f.name);
            let local_path = local_dir.join(&f.name);
            match f.dir {
                true => {
                    a.log("mkdir", &remote_path, None);
                    std::fs::create_dir(&local_path)
                        .map_err(|err| format!("Make dir {:?} fail {}", local_path, err))?;
                    download_dir(a, &remote_path, &local_path)
                }
                false => {
                    a.log("download", &remote_path, Some(f.size));
                    download_file(a, &remote_path, &local_path)
                }
            }
        }
        (None, Some(f)) => {
            let p = local_dir.join(&f.name);
            a.log("rm", &p, None);
            match f.dir {
                true => std::fs::remove_dir_all(&p),
                false => std::fs::remove_file(&p),
            }
            .map_err(|err| format!("rm of {:?} fail {}", p, err))
        }
        (None, None) => Ok(()),
    }
}

fn download_file(a: &Assets, remote_path: &PathBuf, local_path: &PathBuf) -> Result<(), String> {
    let mut remote_file = a
        .sftp
        .open(&remote_path)
        .map_err(|err| format!("Open remote file {:?} fail {}", remote_path, err))?;

    let mut local_file = File::create(&local_path)
        .map_err(|err| format!("Create local file {:?} fail {}", local_path, err))?;

    std::io::copy(&mut remote_file, &mut local_file)
        .map_err(|err| format!("Copy {:?} fail {}", remote_path, err))?;

    Ok(())
}
