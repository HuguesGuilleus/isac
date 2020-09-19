#[macro_use]
extern crate lazy_static;

use ssh2::{Session, Sftp};
use std::fs::{create_dir_all, read_dir, remove_dir_all, File};
use std::path::PathBuf;

mod addr;
pub use addr::{addr_from_reader, Addr};
mod linkvec;
use linkvec::{linkvec, Couple};

mod metafile;
use metafile::MetaFile;

mod assets;
pub use assets::print_err;
use assets::Assets;

pub type R = Result<(), String>;

fn compare_dir<M>(a: &Assets, remote_dir: &PathBuf, local_dir: &PathBuf, m: M) -> R
where
    M: Fn(&Assets, &Couple<MetaFile>, &PathBuf, &PathBuf) -> R,
{
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
                a.err(format!(
                    "In indexing remote directory {:?}: {}",
                    remote_dir, err
                ));
                None
            }
        })
        .collect();

    let local_list: Vec<MetaFile> = read_dir(local_dir)
        .map_err(|err| format!("Index local directory {:?} fail: {}", local_dir, err))?
        .filter_map(|r| match r {
            Ok(f) => Some(f),
            Err(err) => {
                a.err(format!(
                    "In indexing local directory {:?}: {}",
                    local_dir, err
                ));
                None
            }
        })
        .filter_map(|f| match MetaFile::try_from(f) {
            Ok(meta) => Some(meta),
            Err(err) => {
                a.err(format!(
                    "In indexing local directory {:?}: {}",
                    local_dir, err
                ));
                None
            }
        })
        .collect();

    linkvec(remote_list, local_list)
        .iter()
        .map(|couple| m(a, couple, &remote_dir, &local_dir))
        .filter_map(|r| r.err())
        .for_each(|err| a.err(err));

    Ok(())
}

/* PRINT DETAIL */

pub fn list(a: Addr, ansi: bool) -> R {
    if ansi {
        println!(
            "\x1b[1m{:>12} \x1b[32m{} \x1b[0m<-> \x1b[1;34m{:x}\x1b[36m{}\x1b[0m",
            "list", a.digest, a, a.root
        );
    } else {
        println!("{:>12}: {} <-> {}", "list", a.digest, a);
    }

    Ok(())
}

/* UPLOAD */

pub fn upload(a: Addr, ansi: bool) -> R {
    let assets = Assets::new(a, ansi)?;

    upload_dir(
        &assets,
        &PathBuf::from(&assets.a.root),
        &PathBuf::from(&assets.a.digest),
    )
}

fn upload_couple(
    a: &Assets,
    couple: &Couple<MetaFile>,
    remote_dir: &PathBuf,
    local_dir: &PathBuf,
) -> R {
    match couple {
        (Some(remote), Some(local)) => {
            let r = remote_dir.join(&remote.name);
            if local.dir != remote.dir {
                match remote.dir {
                    true => remove_dir(a, &r)?,
                    false => a
                        .sftp
                        .unlink(&r)
                        .map_err(|err| format!("Remove {:?} fail {}", &r, err))?,
                }
                upload_couple(a, &(None, Some(local.clone())), remote_dir, local_dir)
            } else {
                match remote.dir {
                    true => upload_dir(a, &r, &local_dir.join(&local.name)),
                    false => {
                        a.log("keep", &r, Some(remote.size));
                        Ok(())
                    }
                }
            }
        }
        (Some(remote), None) => {
            let r = remote_dir.join(&remote.name);
            match remote.dir {
                true => remove_dir(a, &r),
                false => {
                    a.log("rm", &r, None);
                    a.sftp
                        .unlink(&r)
                        .map_err(|err| format!("Remove file {:?} fail {}", r, err))
                }
            }
        }
        (None, Some(local)) => {
            let r = remote_dir.join(&local.name);
            let l = local_dir.join(&local.name);
            match local.dir {
                true => {
                    a.log("mkdir", &r, None);
                    a.sftp
                        .mkdir(&r, 0o0777)
                        .map_err(|err| format!("Make directory {:?} fail {}", &r, err))?;
                    upload_dir(a, &r, &l)
                }
                false => {
                    a.log("upload", &r, Some(local.size));
                    upload_file(a, &r, &l)
                }
            }
        }
        (None, None) => Ok(()),
    }
}

fn remove_dir(a: &Assets, remote_dir: &PathBuf) -> R {
    a.sftp
        .readdir(&remote_dir)
        .map_err(|err| format!("Read file (to remove it) {:?} fail {}", &remote_dir, err))?
        .iter()
        .map(|(p, stat)| match stat.is_dir() {
            true => remove_dir(a, &p),
            false => {
                a.log("rm", &p, None);
                a.sftp
                    .unlink(&p)
                    .map_err(|err| format!("Remove file {:?} fail {}", &p, err))
            }
        })
        .filter_map(|r| r.err())
        .for_each(|err| a.err(err));

    a.log("rmdir", &remote_dir, None);
    a.sftp
        .rmdir(&remote_dir)
        .map_err(|err| format!("Remove empty directory {:?} fail {}", &remote_dir, err))
}

fn upload_dir(a: &Assets, remote_dir: &PathBuf, local_dir: &PathBuf) -> R {
    compare_dir(a, remote_dir, local_dir, upload_couple)
}

fn upload_file(a: &Assets, remote_path: &PathBuf, local_path: &PathBuf) -> R {
    std::io::copy(
        &mut File::open(local_path)
            .map_err(|err| format!("Open local file {:?} fail {}", local_path, err))?,
        &mut a
            .sftp
            .create(remote_path)
            .map_err(|err| format!("Create remote file {:?} fail {:?}", remote_path, err))?,
    )
    .map_err(|err| format!("Copy of {:?} fail {}", remote_path, err))
    .map(|_| ())
}

/* DOWNLOAD */

pub fn download(a: Addr, ansi: bool) -> R {
    let assets = Assets::new(a, ansi)?;

    create_dir_all(&assets.a.digest)
        .map_err(|err| format!("Create {:?} directory fail: {}", &assets.a.digest, err))?;

    download_dir(
        &assets,
        &PathBuf::from(&assets.a.root),
        &PathBuf::from(&assets.a.digest),
    )
}

fn download_dir(a: &Assets, remote_dir: &PathBuf, local_dir: &PathBuf) -> R {
    compare_dir(a, remote_dir, local_dir, download_couple)
}

fn download_couple(
    a: &Assets,
    couple: &Couple<MetaFile>,
    remote_dir: &PathBuf,
    local_dir: &PathBuf,
) -> R {
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

fn download_file(a: &Assets, remote_path: &PathBuf, local_path: &PathBuf) -> R {
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
