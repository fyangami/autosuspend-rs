use clap::Parser;
use log::{debug, error, info, warn};
use std::{
    env::set_var,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Parser)]
struct Options {
    #[arg(short, long, default_value_t = 5)]
    check_interval: u64,

    #[arg(short, long, default_value_t = 3600)]
    sec_to_suspend: u64,
}

fn main() {
    init_logging();
    let options = Options::parse();
    let mut suspend_at: Option<u64> = None;
    loop {
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        info!("to check is any user logged on");
        if is_any_user_logged_on() {
            info!("got user logged on");
            suspend_at = None;
        } else {
            info!("no user logged on");
            if let Some(suspend_ts) = suspend_at {
                debug!("suspend time: {}, now: {}", suspend_ts, now_ts);
                if suspend_ts <= now_ts {
                    warn!("suspend time was meet, system will be suspend");
                    suspend_at = None;
                    match Command::new("sudo")
                        .arg("/usr/bin/systemctl")
                        .arg("suspend")
                        .output()
                    {
                        Err(e) => {
                            error!("execute suspend command error: {}", e);
                        }
                        Ok(output) => {
                            if !output.status.success() {
                                error!(
                                    "execute suspend command error, stdout: {}, stderr: {}",
                                    String::from_utf8_lossy(&output.stdout),
                                    String::from_utf8_lossy(&output.stderr)
                                );
                            }
                        }
                    }
                }
            } else {
                info!("starting to take account suspend time");
                suspend_at = Some(now_ts + options.sec_to_suspend);
            }
        }
        thread::sleep(Duration::from_secs(options.check_interval));
    }
}

fn is_any_user_logged_on() -> bool {
    match Command::new("who").output() {
        Ok(output) if output.status.success() => {
            if let Ok(output) = String::from_utf8_lossy(&output.stdout).parse::<String>() {
                let output = output.trim();
                if output != "" {
                    let lens = output.split("\n").collect::<Vec<&str>>();
                    debug!("current logged on users: {:?}", lens);
                    return lens.len() > 0;
                }
                return false;
            }
            error!(
                "parse output error, stderr: {:?}, stdout: {:?}",
                output.stdout, output.stderr
            );
        }
        Ok(output) => {
            error!(
                "execute loginctl error, stderr: {:?}, \nstdout: {:?}",
                String::from_utf8(output.stderr),
                String::from_utf8(output.stdout)
            )
        }
        Err(e) => {
            error!("execute loginctl error: {}", e)
        }
    }
    false
}

fn init_logging() {
    if option_env!("RUST_LOG").is_none() {
        set_var("RUST_LOG", "info");
    }
    env_logger::init();
}
