#![feature(termination_trait_lib, process_exitcode_placeholder)]

fn main() -> finalreturn::R {
    let name = "list";

    isac::addr_from_reader(
        std::fs::File::open(name)
            .map_err(|err| format!("Open {:?} fail because: {}", name, err))?,
    )
    .for_each(|a| {
        println!("\x1b[1;44m CONNECT TO \x1b[0m {}", a);
        let before = std::time::Instant::now();
        match isac::update(&a) {
            Err(err) => eprintln!("Error: {}\r\n", err),
            Ok(()) => println!("Done in {:?}", before.elapsed()),
        }
    });

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
