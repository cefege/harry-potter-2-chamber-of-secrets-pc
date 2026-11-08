use std::ffi::OsString;

fn main() {
    std::process::exit(hp2_engine::ucc_owner::run(std::env::args_os().collect::<Vec<OsString>>()));
}
