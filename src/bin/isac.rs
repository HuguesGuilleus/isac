#![feature(termination_trait_lib, process_exitcode_placeholder)]

use std::fs::File;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use threadpool::ThreadPool;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(subcommand)]
    cmd: Command,

    // The key file, if it doesn't exist, the SSH Agent will be used.
    #[structopt(long, default_value = "key")]
    key: String,

    /// The max number of working thread.
    #[structopt(long, default_value = "4")]
    thread: usize,

    /// The list of remote servers.
    ///
    /// The list has the form: `ser@server[:port]/path/to/dir # comment`.
    /// Blank lines and only comment line are permitted.
    #[structopt(short, long, default_value = "list")]
    list: PathBuf,

    /// Disable ANSI char in log.
    #[structopt(long)]
    no_ansi: bool,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// Downolad all files (from the server list).
    Download,
    /// Upload the files if it no exist on the remote server (from the server list).
    Upload,
    /// List all addrs (from the server list).
    List,
    /// Connect to all servers (from the server list).
    Connect,
    /// Init the directory: create key and key.pub if not exist and the list file.
    Init,
}

fn main() -> finalreturn::R {
    let opt = Opt::from_args();
    let l = &opt.list;
    let ansi = !opt.no_ansi;
    let f = match opt.cmd {
        Command::Download { .. } => isac::download,
        Command::Upload { .. } => isac::upload,
        Command::List { .. } => isac::list,
        Command::Connect { .. } => isac::connect,
        Command::Init { .. } => return init(l, &opt.key),
    };

    let key = std::fs::read_to_string("key").ok();

    let pool = ThreadPool::new(if let Command::List { .. } = opt.cmd {
        1
    } else if opt.thread == 0 {
        4
    } else {
        opt.thread
    });

    isac::addr_from_reader(
        File::open(l).map_err(|err| format!("Open {:?} fail because: {}", l, err))?,
    )
    .for_each(|a| {
        let key = key.clone();
        pool.execute(move || {
            if let Err(e) = f(a.clone(), ansi, key) {
                isac::print_err(e, &a, ansi)
            }
        })
    });
    pool.join();

    Ok(())
}

// Generate teh SSH key + the list of remote servers.
fn init(list: &PathBuf, keypath: &str) -> finalreturn::R {
    use osshkeys::{cipher::Cipher, KeyPair, KeyType};
    use std::io::prelude::*;

    if !list.exists() {
        println!("Write {:?} servers list", list);
        File::create(list)
            .map_err(|err| format!("Create {:?} fail: {}", list, err))?
            .write_all(b"# Write one server by line with format: 'user@host[:port]/root'\n")
            .map_err(|err| format!("Write into {:?} fail: {}", list, err))?
    }

    if !Path::new(keypath).exists() {
        // Generate the key
        println!("Generate the key ...");
        let mut key = KeyPair::generate(KeyType::RSA, 4096)
            .map_err(|err| format!("Genrate the key fail: {}", err))?;

        let comment = key.comment_mut();
        comment.push_str("isac@");
        comment.push_str(
            hostname::get()
                .map_err(|err| format!("Fail to get hostname {}", err))?
                .to_str()
                .unwrap_or("localhost"),
        );

        // Private part
        File::create(keypath)
            .map_err(|err| format!("Fail to create {:?} {}", keypath, err))?
            .write_all(
                key.serialize_openssh(None, Cipher::Null)
                    .map_err(|err| format!("Fail to serialize the new key {}", err))?
                    .as_bytes(),
            )
            .map_err(|err| format!("Fail to write the key into {:?}: {}", keypath, err))?;

        // Public part
        let public = key
            .serialize_publickey()
            .map_err(|err| format!("Serialize the public part of the new key fail {}", err))?;
        let p = format!("{}.pub", keypath);

        File::create(&p)
            .map_err(|err| format!("Fail to create {:?} {}", p, err))?
            .write_all(public.as_bytes())
            .map_err(|err| format!("Fail to write the public key into {:?}: {}", p, err))?;

        println!("The new key, public part: \n\n{}\n", public);
    }

    Ok(())
}

mod finalreturn {
    pub type R = Result<(), FinalReturn>;

    pub struct FinalReturn {
        s: String,
    }
    impl std::convert::From<String> for FinalReturn {
        fn from(s: String) -> Self {
            FinalReturn { s: s }
        }
    }
    impl std::fmt::Debug for FinalReturn {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
            f.write_str(&self.s)
        }
    }
    impl std::process::Termination for FinalReturn {
        fn report(self) -> i32 {
            eprintln!("Error: {}", self.s);
            return std::process::ExitCode::FAILURE.report();
        }
    }
}
