// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--sensevoice-worker") {
        let job_file = vtt_keyboard_lib::parse_sensevoice_worker_job_file_arg(&args);
        let code = vtt_keyboard_lib::run_sensevoice_worker(job_file.as_deref());
        std::process::exit(code);
    }
    vtt_keyboard_lib::run()
}
