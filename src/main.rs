use std::{
    fs::metadata,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, SystemTime},
};

fn main() -> ! {
    let settings = XscreensaverSettings::load();
    if !settings.dpms_enabled {
        eprintln!("dpmsOff not configured in ~/.xscreensaver");
        std::process::exit(1);
    }

    let rx = spawn_xscreensaver_watch();

    let mut timer = None;
    let mut locked = None;
    let password_timeout = settings.password_timeout * 3;
    loop {
        if let Ok(status) = rx.recv_timeout(Duration::from_secs(5)) {
            match status {
                _ if status.contains("LOCK") => timer = Some(std::time::Instant::now()),
                // Maybe other events should be processed?
                _ => {
                    locked = None;
                    timer = None;
                }
            }
        }
        match (timer, locked) {
            // Locked, and dpmsOff time has elapsed
            (Some(time), None) if time.elapsed() > settings.dpms_off => {
                println!("Locked, and dpms time has elapsed");
                timer = None;
                locked = Some(());
                suspend();
            }
            // Locked and password_timeout has passed
            (Some(time), Some(_)) if time.elapsed() > password_timeout => {
                println!("Locked and lock timeout has passed");
                timer = None;
                locked = Some(());
                suspend();
            }
            // Woken up but not unlocked
            (None, Some(_)) => {
                println!("Woken up but not unlocked");
                timer = Some(std::time::Instant::now());
            }
            (_, _) => {}
        };
    }
}

/// Suspend the system
fn suspend() {
    if inhibit_suspend() {
        return;
    }
    let _ = Command::new("/usr/bin/systemctl")
        .arg("suspend")
        .spawn()
        .expect("Suspending");
}

/// Don't suspend if a '.no_suspend file was modified in the last 8 hours
/// `touch ~/.no_suspend` to block suspend
fn inhibit_suspend() -> bool {
    let filename = format!(
        "{}/.no_suspend",
        std::env::var("HOME").expect("Get HOME environment variable")
    );
    let no_suspend_lifetime = SystemTime::now()
        .checked_sub(Duration::from_secs((8 * 60 * 60) as u64))
        .expect("Time subtraction");

    metadata(filename)
        .and_then(|m| m.modified())
        .map(|modified| modified >= no_suspend_lifetime)
        .unwrap_or_default()
}

/// Watch Xscreensaver output for events
fn spawn_xscreensaver_watch() -> Receiver<String> {
    let mut xs = Command::new("/usr/bin/xscreensaver-command")
        .arg("-watch")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Running xscreensaver");

    let stdout = xs.stdout.take().expect("Opening stdout");
    let mut lines = BufReader::new(stdout).lines();
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        while let Some(Ok(line)) = lines.next() {
            tx.send(line).unwrap();
        }
    });
    rx
}

/// Settings from ~/.xscreensaver
#[derive(Default, Debug)]
struct XscreensaverSettings {
    /// Is DPMS enabled
    dpms_enabled: bool,
    /// How long until DPMS activates
    dpms_off: Duration,
    /// How long should a password dialog box be left on the screen
    password_timeout: Duration,
}

impl XscreensaverSettings {
    fn load() -> Self {
        let filename = format!(
            "{}/.xscreensaver",
            std::env::var("HOME").expect("Get HOME environment variable")
        );
        let config = std::fs::read_to_string(filename).expect("Read XScreensaver config");
        let mut settings = Self::default();
        for line in config.lines() {
            match line {
                value if line.contains("dpmsEnabled") => {
                    settings.dpms_enabled = XscreensaverSettings::parse_bool(value)
                }
                value if line.contains("dpmsOff") => {
                    settings.dpms_off = XscreensaverSettings::parse_time(value)
                }
                value if line.contains("passwdTimeout") => {
                    settings.password_timeout = XscreensaverSettings::parse_time(value)
                }
                _ => {}
            };
        }
        settings
    }

    /// Parse a bool from the config
    fn parse_bool(line: &str) -> bool {
        line.split(':')
            .last()
            .map(|s| s.trim().to_lowercase().parse().expect("parsing bool"))
            .expect("Get Parsing bool")
    }

    /// Parse a time to a Duration
    fn parse_time(line: &str) -> Duration {
        let time_in_secs = line
            .splitn(2, ':')
            .skip(1)
            .map(|s| {
                s.rsplit(':')
                    .enumerate()
                    .map(|(i, n)| {
                        n.trim().parse::<u64>().expect("Parse time as u64") * (60 * i as u64)
                    })
                    .sum()
            })
            .next()
            .expect("Get time");
        Duration::from_secs(time_in_secs)
    }
}
