#[macro_use]
extern crate lazy_static;

use separator::Separatable;
use ssh2::{FileStat, Session, Sftp};
use std::ffi::OsStr;
use std::fs::{create_dir_all, read_dir, File};
use std::path::PathBuf;

mod addr;
pub use addr::{addr_from_reader, Addr};
mod linkvec;
use linkvec::{linkvec, Couple};

pub fn update(a: &Addr) -> Result<(), String> {
    create_dir_all(&a.digest)
        .map_err(|err| format!("Create {:?} directory fail: {}", a.digest, err))?;
    let sftp = connect(a)?;

    update_dir(&sftp, &PathBuf::from(&a.root), &PathBuf::from(&a.digest))
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

fn update_dir(sftp: &Sftp, remote_dir: &PathBuf, local_dir: &PathBuf) -> Result<(), String> {
    let raw_remote_list = sftp
        .readdir(remote_dir)
        .map_err(|err| format!("Index {:?} directory fail {}", remote_dir, err))?;
    let remote_list: Vec<(&OsStr, &FileStat)> = raw_remote_list
        .iter()
        .filter_map(|(p, stat)| match p.file_name() {
            Some(n) => Some((n, stat)),
            None => None,
        })
        .collect();

    let mut local_list: Vec<std::fs::DirEntry> = Vec::new();
    for r in read_dir(local_dir)
        .map_err(|err| format!("Err when read directory {:?} {}", local_dir, err))?
    {
        local_list
            .push(r.map_err(|err| format!("Err in reading directory {:?} {}", local_dir, err))?)
    }

    linkvec(
        remote_list,
        |(n1, _), (n2, _)| n1.partial_cmp(n2),
        local_list,
        |f1, f2| f1.file_name().partial_cmp(&f2.file_name()),
        |(n, _), f| n.partial_cmp(&f.file_name()).unwrap(),
    )
    .iter()
    .map(|couple| update_couple(sftp, couple, &remote_dir, &local_dir))
    .filter_map(|r| r.err())
    .for_each(|err| eprintln!("\x1b[K\x1b[1;41m ERROR \x1b[0m {}", err));

    Ok(())
}

fn update_couple(
    sftp: &Sftp,
    couple: &Couple<(&OsStr, &ssh2::FileStat), std::fs::DirEntry>,
    remote_dir: &PathBuf,
    local_dir: &PathBuf,
) -> Result<(), String> {
    match couple {
        (Some((n, stat)), Some(f)) => {
            let remote_path = &remote_dir.join(n);
            let p = local_dir.join(f.file_name());
            match (
                stat.is_dir(),
                f.file_type()
                    .map_err(|err| format!("{} on {:?}", err, &p))?
                    .is_dir(),
            ) {
                (true, true) => update_dir(sftp, &remote_path, &p),
                (true, false) => {
                    log("rm", &p, None);
                    std::fs::remove_file(&p)
                        .map_err(|err| format!("Fail to remove {:?}: {}", &p, err))?;
                    update_couple(sftp, &(Some((n, stat)), None), remote_dir, local_dir)
                }
                (false, true) => {
                    log("rmdir", &p, None);
                    std::fs::remove_dir_all(&p)
                        .map_err(|err| format!("Fail to remove directory {:?}: {}", &p, err))?;
                    log("download", &remote_path, stat.size);
                    update_file(sftp, &remote_path, &p)
                }
                (false, false) => {
                    match (
                        stat.mtime,
                        f.metadata()
                            .map_err(|err| {
                                format!("Error when get metadata of {:?}: {}", f.file_name(), err)
                            })?
                            .modified(),
                    ) {
                        (Some(s), Ok(st)) => {
                            if s > st
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .unwrap()
                                // .unwrap_or(std::time::Duration::zero())
                                .as_secs()
                            {
                                log("download", &remote_path, stat.size);
                                update_file(sftp, &remote_path, &p)
                            } else {
                                log("ok", &remote_path, None);
                                Ok(())
                            }
                        }
                        _ => {
                            log("download", &remote_path, stat.size);
                            update_file(sftp, &remote_path, &p)
                        }
                    }
                }
            }
        }
        (Some((r, stat)), None) => {
            let local_path = local_dir.join(r);
            let remote_path = remote_dir.join(r);
            if stat.is_dir() {
                log("mkdir", &remote_path, None);
                std::fs::create_dir(&local_path).map_err(|err| {
                    format!(
                        "create the directory {:?} fail: {}\r\n\x1b[1G\x1b[1F",
                        &local_path, err
                    )
                })?;
                update_dir(sftp, &remote_path, &local_path)
            } else {
                log("download", &remote_path, stat.size);
                update_file(sftp, &remote_path, &local_path)
            }
        }
        (None, Some(f)) => {
            let p = local_dir.join(f.file_name());
            log("rm", &p, None);
            match f
                .file_type()
                .map_err(|e| format!("{} on {:?}", e, &p))?
                .is_dir()
            {
                true => std::fs::remove_dir_all(&p),
                false => std::fs::remove_file(&p),
            }
            .map_err(|err| format!("rm of {:?} fail: {}", p, err))
        }
        (None, None) => Ok(()),
    }
}

fn update_file(sftp: &Sftp, remote_path: &PathBuf, local_path: &PathBuf) -> Result<(), String> {
    let mut remote_file = sftp
        .open(&remote_path)
        .map_err(|err| format!("Remote file {:?} fail {}", remote_path, err))?;

    let mut local_file = File::create(&local_path)
        .map_err(|err| format!("Create local file {:?} fail: {}", &local_path, err))?;

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
