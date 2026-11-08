mod boot_services;
mod fire_probe;
mod gate_reporters;
mod reporters;
mod runtime_trace;

mod main_loop_services;
use boot_services::BootHost;
use hp2_launcher::boot;
use std::ffi::OsString;

fn arguments_without_seed() -> Vec<OsString> {
    let mut args = std::env::args_os();
    let mut retained = Vec::new();
    if let Some(program) = args.next() {
        retained.push(program);
    }
    for argument in args {
        if let Some(text) = argument.to_str() {
            if let Some(seed) = text.strip_prefix("--rng-seed=") {
                unsafe {
                    std::env::set_var("HP2_RNG_SEED", seed);
                }
                continue;
            }
        }
        retained.push(argument);
    }
    retained
}

fn main() {
    let args = arguments_without_seed();
    let Some(args) = args
        .iter()
        .map(|argument| argument.to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
    else {
        eprintln!("hp2_game: command-line arguments must be valid UTF-8");
        std::process::exit(2);
    };
    let mut host = Box::new(BootHost::new(args));
    let services = host.services();
    let initial = unsafe { boot::run_main(&services) };
    let (exit_code, run_finish_process) = match initial {
        Ok(Some(code)) => (code, false),
        Err(status) => (host.fail_status(status), false),
        Ok(None) => {
            if let Err(status) = unsafe { boot::run_boot_pre_try(&services) } {
                (host.fail_status(status), false)
            } else {
                let result = unsafe { boot::run_after_launcher(&services) };
                let code = result.unwrap_or_else(|status| host.fail_status(status));
                (code, true)
            }
        }
    };
    let teardown_failed = if run_finish_process {
        host.end_loop_scope();
        host.finish()
    } else {
        false
    };
    std::process::exit(boot::finish(teardown_failed, exit_code));
}
