use crate::Opt;
use notify::{
    event::{Event as NEvent, EventKind as NEventKind},
    immediate_watcher, RecursiveMode, Watcher,
};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpStream;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct State {
    pub link_quality: u8,
    pub ping: f32,
    pub all_profiles: Arc<Mutex<Vec<String>>>,
    pub active_profile: Option<String>,
}

pub fn inotify_watch(
    all_profiles: Arc<Mutex<Vec<String>>>,
    dir: &str,
) -> Result<(), notify::Error> {
    // initialize the inotify watcher
    let mut watcher = immediate_watcher(move |res: Result<NEvent, _>| match res {
        Ok(event) => {
            match event.kind {
                NEventKind::Create(_) => {
                    // Add the new profile
                    for path in event.paths {
                        match path.file_name().unwrap().to_str() {
                            Some(p) => all_profiles.lock().unwrap().push(p.to_owned()),
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
                            Some(p) => all_profiles.lock().unwrap().remove_item(&p),
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
    })?;

    watcher.watch(dir, RecursiveMode::Recursive)?;

    Ok(())
}
// Scans the files in /etc/netctl and adds the profiles to the vector
pub fn scan_profiles(all_profiles: &mut Vec<String>) -> Result<(), std::io::Error> {
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
pub fn update_state(state: &mut State, args: &Opt) -> Result<(), std::io::Error> {
    // get the current active profile
    #[cfg(not(feature = "auto"))]
    let raw_profiles = Command::new("netctl").arg("list").output()?;
    #[cfg(feature = "auto")]
    let raw_profiles = Command::new("netctl-auto").arg("list").output()?;
    // Iterate through each line
    let mut active_profile = None;
    for line in raw_profiles.stdout.split(|c| *c == '\n' as u8) {
        if line.len() == 0 {
            continue;
        }
        // If the line starts with an asterisk, then the profile is active
        // and we need it's name
        if line[0] == '*' as u8 {
            active_profile = match std::str::from_utf8(&line[2..]) {
                Ok(s) => Some(s.to_owned()),
                Err(e) => {
                    eprintln!("Can't read profile name from netctl list: {:?}", e);
                    break;
                }
            };
            break;
        }
    }
    state.active_profile = active_profile;

    if let Some(active_profile) = &state.active_profile {
        // Now we need to get the interface the current profile uses
        let mut current_profile_file = File::open(&format!("/etc/netctl/{}", active_profile))?;
        let mut current_profile_contents = String::new();
        current_profile_file.read_to_string(&mut current_profile_contents)?;
        // iterate over lines to find the one specifying the interface
        let mut profile_interface = "";
        for line in current_profile_contents.split('\n') {
            if line.starts_with("Interface") {
                let mut interface = match line.split('=').nth(1) {
                    Some(i) => i,
                    None => {
                        eprintln!(
                            "Profile not properly configured! Corrupted file: /etc/netctl/{}",
                            active_profile
                        );
                        continue;
                    }
                }
                .trim();
                // Remove quotes if there
                if interface.starts_with('"') && interface.ends_with('"') {
                    // double quotes
                    interface = &interface[1..interface.len() - 1];
                } else if interface.starts_with('\'') && interface.ends_with('\'') {
                    // single quotes
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
    }

    // check ping
    // try connecting to the given IP
    let now = Instant::now();
    if TcpStream::connect_timeout(&args.host, Duration::from_nanos(500_000_000)).is_ok() {
        state.ping = now.elapsed().as_millis() as f32;
    } else {
        state.ping = f32::INFINITY;
    }

    Ok(())
}
