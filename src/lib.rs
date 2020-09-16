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

pub fn donwload(a: &Addr) -> Result<(), String> {
    create_dir_all(&a.digest)
        .map_err(|err| format!("Create {:?} directory fail: {}", a.digest, err))?;

    download_dir(
        &connect(a)?,
        &PathBuf::from(&a.root),
        &PathBuf::from(&a.digest),
    )
}

fn connect(a: &Addr) -> Result<Sftp, String> {
    let mut s =
        Session::new().map_err(|err| format!("The creation of a new SSH session fail: {}", err))?;
    s.set_compress(true);
    s.set_tcp_stream(a.connect()?);
    s.handshake()
        .map_err(|err| format!("SSH Handshake fail for {}: {}", a, err))?;
    s.userauth_agent(&a.user)
        .map_err(|err| format!("Authentification fail for {}: {}", a, err))?;

    s.sftp()
        .map_err(|err| format!("Open SFTP fail for {}: {}", a, err))
}

fn download_dir(sftp: &Sftp, remote_dir: &PathBuf, local_dir: &PathBuf) -> Result<(), String> {
    use std::convert::TryFrom;

    log("Index", remote_dir, None);

    let remote_list: Vec<MetaFile> = sftp
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
                eprintln!("ERROR in locla directory {:?}: {} ", local_dir, err);
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
        .map(|couple| download_couple(sftp, couple, &remote_dir, &local_dir))
        .filter_map(|r| r.err())
        .for_each(|err| eprintln!("\x1b[K\x1b[1;41m ERROR \x1b[0m {}", err));

    Ok(())
}

fn download_couple(
    sftp: &Sftp,
    couple: &Couple<MetaFile>,
    remote_dir: &PathBuf,
    local_dir: &PathBuf,
) -> Result<(), String> {
    match couple {
        (Some(remote), Some(local)) => {
            let remote_path = remote_dir.join(&remote.name);
            let local_path = local_dir.join(&local.name);
            match (remote.dir, local.dir) {
                (true, true) => download_dir(sftp, &remote_path, &local_path),
                (true, false) => {
                    log("rm", &local_path, None);
                    std::fs::remove_file(&local_path)
                        .map_err(|err| format!("Remove {:?} fail: {}", local_path, err))?;
                    download_couple(sftp, &(Some(remote.clone()), None), remote_dir, local_dir)
                }
                (false, true) => {
                    log("rmdir", &local_dir, None);
                    remove_dir_all(&local_path)
                        .map_err(|err| format!("Remove dir {:?} fail {}", local_path, err))?;
                    log("download", &remote_path, Some(remote.size));
                    download_file(sftp, &remote_path, &local_path)
                }
                (false, false) => {
                    if remote.mtime < local.mtime {
                        return Ok(());
                    }
                    log("download", &remote_path, Some(remote.size));
                    download_file(sftp, &remote_path, &local_path)
                }
            }
        }
        (Some(f), None) => {
            let remote_path = remote_dir.join(&f.name);
            let local_path = local_dir.join(&f.name);
            match f.dir {
                true => {
                    log("mkdir", &remote_path, None);
                    std::fs::create_dir(&local_path)
                        .map_err(|err| format!("Make dir {:?} fail {}", local_path, err))?;
                    download_dir(sftp, &remote_path, &local_path)
                }
                false => {
                    log("download", &remote_path, Some(f.size));
                    download_file(sftp, &remote_path, &local_path)
                }
            }
        }
        (None, Some(f)) => {
            let p = local_dir.join(&f.name);
            log("rm", &p, None);
            match f.dir {
                true => std::fs::remove_dir_all(&p),
                false => std::fs::remove_file(&p),
            }
            .map_err(|err| format!("rm of {:?} fail {}", p, err))
        }
        (None, None) => Ok(()),
    }
}

fn download_file(sftp: &Sftp, remote_path: &PathBuf, local_path: &PathBuf) -> Result<(), String> {
    let mut remote_file = sftp
        .open(&remote_path)
        .map_err(|err| format!("Open remote file {:?} fail {}", remote_path, err))?;

    let mut local_file = File::create(&local_path)
        .map_err(|err| format!("Create local file {:?} fail {}", local_path, err))?;

    std::io::copy(&mut remote_file, &mut local_file)
        .map_err(|err| format!("Copy {:?} fail {}", remote_path, err))?;

    Ok(())
}

fn log(op: &str, path: &PathBuf, size: Option<u64>) {
    print!(
        "\x1b[K{:>12} \x1b[1;35m{}\x1b[0m",
        op,
        path.to_str().unwrap_or("")
    );
    match size {
        Some(size) => print!(" {} o", size.separated_string()),
        None => {}
    }
    println!("")
    // print!("\r\n\x1b[1G\x1b[1F");
}
