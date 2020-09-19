#![feature(termination_trait_lib, process_exitcode_placeholder)]
use std::path::PathBuf;
use structopt::StructOpt;
use threadpool::ThreadPool;

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(subcommand)]
    cmd: Command,

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
    };

    let pool = ThreadPool::new(if let Command::List { .. } = opt.cmd {
        1
    } else if opt.thread == 0 {
        4
    } else {
        opt.thread
    });

    isac::addr_from_reader(
        std::fs::File::open(l).map_err(|err| format!("Open {:?} fail because: {}", l, err))?,
    )
    .for_each(|a| {
        pool.execute(move || {
            if let Err(e) = f(a.clone(), ansi) {
                isac::print_err(e, &a, ansi)
            }
        })
    });
    pool.join();

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
