//! Process adapter used only by release protocol compatibility qualification.

#![forbid(unsafe_code)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(gd_pages::command::run(&gd_pages::command::Program::default(), &args));
}
