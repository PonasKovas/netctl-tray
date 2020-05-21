#![feature(vec_remove_item)]

use notify::{
    event::{Event as NEvent, EventKind as NEventKind},
    immediate_watcher, RecursiveMode, Watcher,
};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "netctl-tray",
    about = "A lightweight netctl tray app with notifications."
)]
struct Opt {
    /// Tray icon update interval (seconds)
    #[structopt(short, default_value = "2")]
    interval: f32,
    /// Host IP to connect to when checking ping
    #[structopt(long, default_value = "1.1.1.1:53")]
    host: SocketAddr, //Option<SocketAddr>,
}

#[derive(Debug)]
struct State {
    link_quality: u8,
    ping: f32,
    all_profiles: Arc<Mutex<Vec<String>>>,
    active_profile: String,
}

fn main() {
    // Parse arguments
    let args = Opt::from_args();

    let mut state = State {
        link_quality: 0,
        ping: 0.0,
        all_profiles: Arc::new(Mutex::new(Vec::new())),
        active_profile: String::new(),
    };

    // Do the initial profiles scan
    if let Err(e) = scan_profiles(&mut *state.all_profiles.lock().unwrap()) {
        eprintln!("Error while scanning profiles: {:?}", e);
        return;
    }

    let all_profiles_clone = state.all_profiles.clone();

    // initialize the inotify watcher
    let mut watcher = match immediate_watcher(move |res: Result<NEvent, _>| match res {
        Ok(event) => {
            match event.kind {
                NEventKind::Create(_) => {
                    // Add the new profile
                    for path in event.paths {
                        match path.file_name().unwrap().to_str() {
                            Some(p) => all_profiles_clone.lock().unwrap().push(p.to_owned()),
                            None => {
                                eprintln!(
                                    "Can't convert OsStr to str: {:?}",
                                    path.file_name().unwrap()
                                );
                                continue;
                            }
                        };
                    }
                }
                NEventKind::Remove(_) => {
                    // Remove the profile
                    for path in event.paths {
                        match path.file_name().unwrap().to_str() {
                            Some(p) => all_profiles_clone.lock().unwrap().remove_item(&p),
                            None => {
                                eprintln!(
                                    "Can't convert OsStr to str: {:?}",
                                    path.file_name().unwrap()
                                );
                                continue;
                            }
                        };
                    }
                }
                _ => {}
            }
        }
        Err(e) => eprintln!("watch error: {:?}", e),
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error initializing watcher: {:?}", e);
            return;
        }
    };

    if let Err(e) = watcher.watch("/etc/netctl", RecursiveMode::Recursive) {
        eprintln!("error watching: {:?}", e);
    }

    loop {
        let start = Instant::now();

        if let Err(e) = update_state(&mut state, args.host) {
        	eprintln!("Can't update tray state: {:?}", e);
        }
        println!("{:?}", state);
        sleep(
            Duration::from_secs_f32(args.interval)
                .checked_sub(start.elapsed())
                .unwrap_or(Duration::new(0, 0)),
        );
    }
}

// Scans the files in /etc/netctl and adds the profiles to the vector
fn scan_profiles(all_profiles: &mut Vec<String>) -> Result<(), std::io::Error> {
    // for every file or folder in /etc/netcl
    for entry in fs::read_dir("/etc/netctl/")? {
        let path = entry?.path();
        let metadata = path.metadata()?;
        if metadata.is_file() {
            // the file name of the profile configuration
            // is the name of the profile.
            let profile_name = match path.file_name().unwrap().to_str() {
                Some(f) => f,
                None => {
                    eprintln!(
                        "Can't convert OsStr to str: {:?}",
                        path.file_name().unwrap()
                    );
                    continue;
                }
            };
            // add the profile to the vector
            all_profiles.push(profile_name.to_owned());
        }
    }
    Ok(())
}

// Updates the netctl-tray state: ping, quality and current active profile
fn update_state(state: &mut State, ip: SocketAddr) -> Result<(), std::io::Error> {
    // get the current active profile
    let raw_profiles = Command::new("netctl").arg("list").output()?;
    // Iterate through each line
    for line in raw_profiles.stdout.split(|c| *c == '\n' as u8) {
        if line.len() == 0 {
            continue;
        }
        // If the line starts with an asterisk, then the profile is active
        // and we need it's name
        if line[0] == '*' as u8 {
            state.active_profile = match std::str::from_utf8(&line[2..]) {
                Ok(s) => s.to_owned(),
                Err(e) => {
                    eprintln!("Can't read profile name from netctl list: {:?}", e);
                    break;
                }
            };
        }
    }

    // Now we need to get the interface the current profile uses
    let mut current_profile_file = File::open(&format!("/etc/netctl/{}", state.active_profile))?;
    let mut current_profile_contents = String::new();
    current_profile_file.read_to_string(&mut current_profile_contents)?;
    // iterate over lines to find the one specifying the interface
    let mut profile_interface = "";
    for line in current_profile_contents.split('\n') {
        if line.starts_with("Interface") {
            // This is hacky but should work with sane profiles
            let mut interface = match line.split('=').nth(1) {
                Some(i) => i,
                None => {
                    eprintln!(
                        "Profile not properly configured! Corrupted file: /etc/netctl/{}",
                        state.active_profile
                    );
                    continue;
                }
            }
            .trim();
            // Remove quotes if there
            if interface.starts_with('"') && interface.ends_with('"') {
                interface = &interface[1..interface.len() - 1];
            } else if interface.starts_with('\'') && interface.ends_with('\'') {
                interface = &interface[1..interface.len() - 1];
            }
            profile_interface = interface;
            break;
        }
    }

    // Now, as we know the used interface we can check the link quality
    // It can be found in /proc/net/wireless
    let mut file = File::open("/proc/net/wireless")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    // iterate over lines and find the one describing our needed interface
    for line in contents.split('\n').skip(2) {
        if line.starts_with(profile_interface) {
            // Found the line
            // find the right column
            let mut columns = line.split(' ').filter(|x| !x.is_empty());
            let mut link_quality = columns.nth(2).unwrap();
            // remove the last char which is a dot apparently
            link_quality = &link_quality[..link_quality.len() - 1];
            let link_quality: u8 = link_quality.parse().unwrap();
            state.link_quality = link_quality;
        }
    }

    // and the last thing to do is to check ping
    // try connecting to the given IP
    let now = Instant::now();
    if TcpStream::connect_timeout(&ip, Duration::from_nanos(500_000_000)).is_ok() {
        state.ping = now.elapsed().as_millis() as f32;
    } else {
        state.ping = f32::INFINITY;
    }
    Ok(())
}