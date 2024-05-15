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
    
    /// to check activity interval
    #[arg(short = 'i', default_value_t = 60)]
    check_interval: u64,

    /// how long to suspend after no user activity
    #[arg(short = 's', default_value_t = 3600)]
    sec_to_suspend: u64,

    /// force to shuting down when suspend failed
    #[arg(short = 'f', default_value_t = false)]
    force_shutdown: bool,

    /// execute suspend action
    #[arg(short = 'c', default_value = "/usr/bin/systemctl suspend")]
    suspend_command: String,
}

const ASSUME_SUSPEND_FAILED_TOLERANCE: u64 = 3;
const FORCE_SHUTDOWN_TIMEOUT: u64 = 60;

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
            match must_to_command(&options.suspend_command).output() {
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
                    info!("system will be force shutdown at 60s");
                    thread::sleep(Duration::from_secs(FORCE_SHUTDOWN_TIMEOUT));
                    if is_any_user_logged_on() {
                        warn!("detected user activity, stop to shutdown machine");
                    } else {
                        warn!("force to shuting down machine");
                        Command::new("/usr/bin/systemctl").arg("poweroff").exec();
                    }
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

fn must_to_command(str_cmd: &str) -> Command  {
    let mut splits = str_cmd.trim().split_whitespace();
    let cmd = splits.next().expect("command is missing");
    let mut cmd = Command::new(cmd);
    cmd.args(splits.collect::<Vec<&str>>());
    cmd
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

#[cfg(test)]
mod tests {
    use crate::must_to_command;


    #[test]
    pub fn test_must_to_command() {
        must_to_command("ls");
        must_to_command("ls -alh");
        must_to_command("/usr/bin/ls -l -a -h");
    }

    #[test]
    #[should_panic]
    pub fn test_panic_must_to_command() {
        must_to_command("    ");
    }
}
