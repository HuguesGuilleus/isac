use super::{Addr, PathBuf, Session, Sftp};
use separator::Separatable;
use std::time::Instant;

pub struct Assets {
    pub a: Addr,
    pub sftp: Sftp,
    pub ansi: bool,
    pub before: Instant,
}
impl Assets {
    pub fn new(a: Addr, ansi: bool) -> Result<Assets, String> {
        Ok(Assets {
            ansi: ansi,
            sftp: Assets::connect(&a)?,
            a: a,
            before: Instant::now(),
        })
    }
    pub fn connect(a: &Addr) -> Result<Sftp, String> {
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
    pub fn ms(&self, op: &str, ms: &str) {
        if self.ansi {
            println!(
                "\x1b[1m{:>12} \x1b[1;34m{:x}\x1b[36m{}\x1b[0m {}",
                op, self.a, self.a.root, ms
            )
        } else {
            println!("{:>12}: <{:x}> {} {}", op, self.a, self.a.root, ms)
        }
    }
    pub fn log(&self, op: &str, path: &PathBuf, size: Option<u64>) {
        let p = path.to_str().unwrap_or("");

        if self.ansi {
            print!(
                "\x1b[1m{:>12} \x1b[1;34m{:x}\x1b[36m{}\x1b[0m",
                op, self.a, p
            );
        } else {
            print!("{:>12}: <{:x}> {}", op, self.a, p);
        }

        match size {
            Some(size) => print!(" ({} o)", size.separated_string()),
            None => {}
        }

        println!("");
    }
    pub fn err(&self, err: String) {
        print_err(err, &self.a, self.ansi);
    }
}

pub fn print_err(err: String, a: &Addr, ansi: bool) {
    match ansi {
        true => eprintln!(
            "\x1b[1;31m{:>12} \x1b[1;33m{:x} \x1b[31m{}\x1b[0m",
            "ERROR", a, err
        ),
        false => eprintln!("{:>12}: <{:x}> {}", "ERROR", a, err),
    }
}

impl Drop for Assets {
    fn drop(&mut self) {
        self.ms("DONE", &format!("in {:?}", self.before.elapsed()))
    }
}
