use clap::Parser;
use log::{debug, error, info, warn};
use std::{
    os::unix::process::CommandExt,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Parser)]
struct Options {
    #[arg(short, long, default_value_t = 60)]
    check_interval: u64,

    #[arg(short, long, default_value_t = 3600)]
    sec_to_suspend: u64,

    #[arg(short, long, default_value_t = false)]
    force_shutdown: bool,
}

const ASSUME_SUSPEND_FAILED_TOLERANCE: u64 = 3;

fn main() {
    env_logger::init();
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
            thread::sleep(Duration::from_secs(options.check_interval));
            continue;
        }
        info!("no user logged on");
        suspend_at.get_or_insert_with(|| {
            let at = now_ts + options.sec_to_suspend;
            info!("system will be suspended at: {}", at);
            return at;
        });
        let at = suspend_at.get_or_insert(now_ts + options.sec_to_suspend);
        debug!("suspend time: {}, now: {}", at, now_ts);
        let mut how_long_to_suspend = *at - now_ts;
        if how_long_to_suspend <= 0 {
            warn!("suspend time was meet, system will be suspending");
            suspend_at = None;
            match Command::new("/usr/bin/systemctl").arg("suspend").output() {
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
            let after_suspend = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if after_suspend - now_ts < ASSUME_SUSPEND_FAILED_TOLERANCE {
                // `systemctl suspend` may execute failed but no error returned
                error!("instantaneous woken from suspend, assume suspend failed!");
                if options.force_shutdown {
                    thread::sleep(Duration::from_secs(3));
                    warn!("force to shuting down machine");
                    Command::new("/usr/bin/systemctl").arg("poweroff").exec();
                }
            }
            continue;
        }
        if how_long_to_suspend > options.check_interval {
            how_long_to_suspend = options.check_interval
        }
        thread::sleep(Duration::from_secs(how_long_to_suspend));
    }
}

fn is_any_user_logged_on() -> bool {
    match Command::new("w").arg("-i").arg("-h").output() {
        Ok(output) if output.status.success() => {
            if let Ok(output) = String::from_utf8_lossy(&output.stdout).parse::<String>() {
                let output = output.trim();
                if output != "" {
                    let lens = output.split("\n").collect::<Vec<&str>>();
                    debug!("current logged on users: \n\n{:?}", lens);
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
