#![recursion_limit = "128"]

extern crate cast;
extern crate clap;
extern crate either;
#[macro_use]
extern crate error_chain;
extern crate inflections;
#[macro_use]
extern crate quote;
extern crate svd_parser as svd;
extern crate syn;

mod errors;
mod generate;
mod util;

use std::fs::File;
use std::process;
use std::io::{self, Write};

use clap::{App, Arg};

use errors::*;
use util::{build_rs, Target};

fn run() -> Result<()> {
    use std::io::Read;

    let matches = App::new("svd2rust")
        .about("Generate a Rust API from SVD files")
        .arg(
            Arg::with_name("input_deprecated")
                .help("(DEPRECATED) Input SVD file")
                .short("i"),
        )
        .arg(
            Arg::with_name("input")
                .help("Input SVD files")
                .multiple(true),
        )
        .arg(
            Arg::with_name("target")
                .long("target")
                .help("Target architecture")
                .takes_value(true)
                .value_name("ARCH"),
        )
        .arg(
            Arg::with_name("nightly_features")
                .long("nightly")
                .help("Enable features only available to nightly rustc")
        )
        .version(concat!(
            env!("CARGO_PKG_VERSION"),
            include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))
        ))
        .get_matches();

    let target = matches
        .value_of("target")
        .map(|s| Target::parse(s))
        .unwrap_or(Ok(Target::CortexM))?;

    let mut xmls: Vec<String> = Vec::new();
    match matches.values_of("input") {
        Some(files) => {
            for file in files {
                let xml = &mut String::new();
                File::open(file)
                    .chain_err(|| "couldn't open the SVD file")?
                    .read_to_string(xml)
                    .chain_err(|| "couldn't read the SVD file")?;
                xmls.push(xml.to_owned());
            }
        }
        None => {
            // TODO: parse multiple concatenated SVDs
            let mut xml = &mut String::new();
            let stdin = std::io::stdin();
            stdin
                .lock()
                .read_to_string(xml)
                .chain_err(|| "couldn't read from stdin")?;
            xmls.push(xml.to_owned());
        }
    }

    let mut devices : Vec<svd::Device> = Vec::new();
    for xml in xmls {
        let device = svd::parse(xml.as_ref());
        devices.push(device.to_owned());
    }


    let nightly = matches.is_present("nightly_features");

    let mut device_x = String::new();
    let items = generate::device::render(&devices.first().unwrap(), target, nightly, &mut device_x)?;

    writeln!(File::create("lib.rs").unwrap(), "{}", quote!(#(#items)*)).unwrap();

    if target == Target::CortexM {
        writeln!(File::create("device.x").unwrap(), "{}", device_x).unwrap();
        writeln!(File::create("build.rs").unwrap(), "{}", build_rs()).unwrap();
    }

    Ok(())
}

fn main() {
    use std::io::Write;

    if let Err(ref e) = run() {
        let stderr = io::stderr();
        let mut stderr = stderr.lock();

        writeln!(stderr, "error: {}", e).ok();

        for e in e.iter().skip(1) {
            writeln!(stderr, "caused by: {}", e).ok();
        }

        if let Some(backtrace) = e.backtrace() {
            writeln!(stderr, "backtrace: {:?}", backtrace).ok();
        } else {
            writeln!(stderr, "note: run with `RUST_BACKTRACE=1` for a backtrace").ok();
        }

        process::exit(1);
    }
}
