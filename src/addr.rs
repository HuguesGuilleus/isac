use md5::{Digest, Md5};
use regex::Regex;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::Write as fmtWrite;
use std::net::TcpStream;

#[derive(Debug, PartialEq, Clone)]
pub struct Addr {
    pub user: String,
    pub host: String,
    pub port: Option<u16>,
    pub root: String,
    pub digest: String,
}

impl Addr {
    pub fn connect(&self) -> Result<TcpStream, String> {
        let mut a = self.host.to_string();
        a.push_str(":");
        match self.port {
            Some(p) => a.push_str(&p.to_string()),
            None => a.push_str("22"),
        };
        TcpStream::connect(&a).map_err(|err| format!("Fail to connect to {:?}: {}", a, err))
    }
}

impl std::convert::TryFrom<&str> for Addr {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r"(?P<u>\w+)@(?P<h>\[[0-9a-f:]+\]|[^:/]+)(:(?P<p>\d{1,4}))?(?P<r>/.+)")
                    .unwrap();
        }
        if !RE.is_match(s) {
            return Err(format!("{:?} no match 'user@host[:port]/root'", s));
        }
        let port = RE.replace(s, "$p").to_string();

        let mut d = String::with_capacity(32);
        for u in Md5::digest(s.as_bytes()).as_slice().iter() {
            write!(&mut d, "{:02x}", u).unwrap();
        }

        Ok(Addr {
            user: RE.replace(s, "$u").to_string(),
            host: RE.replace(s, "$h").to_string(),
            port: if port.len() == 0 {
                None
            } else {
                Some(port.parse().unwrap())
            },
            root: RE.replace(s, "$r").to_string(),
            digest: d,
        })
    }
}
#[test]
fn addr_try_from() {
    let mut a = Addr {
        user: "superuser".to_string(),
        host: "host.net".to_string(),
        port: Some(22),
        root: "/home/u/dir/".to_string(),
        digest: "1210b4c0432588ea4c9beefbb7b2278e".to_string(),
    };
    assert_eq!(Addr::try_from(a.to_string().as_str()).unwrap(), a);

    a.host = "[2001:7fd::1]".to_string();
    a.digest = "e3b34bfc7de97f6f232746455705f093".to_string();
    assert_eq!(Addr::try_from(a.to_string().as_str()).unwrap(), a);

    a.port = None;
    a.digest = "fb65b21bd458eb6de6d3cd34264850fd".to_string();
    assert_eq!(Addr::try_from(a.to_string().as_str()).unwrap(), a);
}

pub fn addr_from_reader<R: std::io::Read>(r: R) -> impl std::iter::Iterator<Item = Addr> {
    use std::io::prelude::*;
    std::io::BufReader::new(r)
        .lines()
        .take_while(|r| r.is_ok())
        .filter_map(|r| r.ok())
        .map(|s| {
            s[..match s.find('#') {
                Some(l) => l,
                None => s.len(),
            }]
                .trim()
                .to_string()
        })
        .enumerate()
        .filter(|(_, l)| l.len() > 0)
        .filter_map(|(i, l)| match Addr::try_from(l.as_str()) {
            Err(err) => {
                eprintln!("line {}: {} ", i + 1, err);
                None
            }
            Ok(a) => Some(a),
        })
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.port {
            Some(p) => write!(f, "{}@{}:{}{}", self.user, self.host, p, self.root),
            None => write!(f, "{}@{}{}", self.user, self.host, self.root),
        }
    }
}
#[test]
fn addr_display() {
    let mut a = Addr {
        user: "u".to_string(),
        host: "h".to_string(),
        port: Some(22),
        root: "/home/u/dir/".to_string(),
        digest: "".to_string(),
    };
    assert_eq!(&format!("addr: {}", &a), "addr: u@h:22/home/u/dir/");
    a.port = None;
    assert_eq!(&format!("addr: {}", &a), "addr: u@h/home/u/dir/");
}
