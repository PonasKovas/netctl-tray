#![feature(vec_remove_item)]

mod state;

use notify_rust::{Notification, Timeout};
use state::{inotify_watch, scan_profiles, update_state, State};
use std::ffi::OsStr;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use users::{get_current_gid, get_current_username, get_user_groups};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "netctl-tray",
    about = "A lightweight netctl tray app with notifications."
)]
pub struct Opt {
    /// Tray icon update interval (seconds)
    #[structopt(short, long, default_value = "2")]
    pub interval: f32,
    /// Host IP to connect to when checking ping
    #[structopt(long, default_value = "1.1.1.1:53")]
    pub host: SocketAddr,
    /// Disables desktop notifications on profile start/stop
    #[structopt(short)]
    pub disable_notifications: bool,
}

fn main() {
    // Parse arguments
    let args = Opt::from_args();

    // Then make sure the user running this is in the group `wheel`
    if !is_user_in_wheel() {
        eprintln!("Warning! You are not in group 'wheel', netctl-tray might not work or work only partially.");
    }
    let mut state = State {
        link_quality: 0,
        ping: 0.0,
        all_profiles: Arc::new(Mutex::new(Vec::new())),
        active_profile: None,
    };

    // Do the initial profiles scan
    if let Err(e) = scan_profiles(&mut *state.all_profiles.lock().unwrap()) {
        eprintln!("Error while scanning profiles: {:?}", e);
        return;
    }

    let all_profiles_clone = state.all_profiles.clone();

    // watch the directory for new/deleted profiles
    if let Err(e) = inotify_watch(all_profiles_clone, "/etc/netctl") {
        eprintln!("Error watching /etc/netctl: {:?}", e);
        return;
    }

    // initialize state
    if let Err(e) = update_state(&mut state, &args) {
        eprintln!("Can't update tray state: {:?}", e);
    }

    loop {
        let start = Instant::now();

        let old_active_profile = state.active_profile.clone();
        if let Err(e) = update_state(&mut state, &args) {
            eprintln!("Can't update tray state: {:?}", e);
        }
        if let Err(e) = profile_notification(&mut state, old_active_profile) {
            eprintln!("Error sending desktop notification: {:?}", e);
        }
        println!("{:?}", state);
        sleep(
            Duration::from_secs_f32(args.interval)
                .checked_sub(start.elapsed())
                .unwrap_or(Duration::new(0, 0)),
        );
    }
}

fn profile_notification(
    state: &mut State,
    old_active_profile: Option<String>,
) -> Result<(), notify_rust::error::Error> {
    // If active profile changed, show notification
    let text = match (&old_active_profile, &state.active_profile) {
        (None, Some(new)) => {
            // Profile started
            format!("Profile <b>{}</b> started.", new)
        }
        (Some(old), None) => {
            // Profile stopped
            format!("Profile <b>{}</b> stopped.", old)
        }
        (Some(old), Some(new)) => {
            if old != new {
                // Profile switched
                format!("Profile switched: from <b>{}</b> to <b>{}</b>.", old, new)
            } else {
                // the same profile; dont send notification
                return Ok(());
            }
        }
        _ => {
            // still no profile; dont send notification
            return Ok(());
        }
    };
    // Send notification
    Notification::new()
        .summary("netctl")
        .body(&text)
        .icon("network-wireless")
        .timeout(Timeout::Milliseconds(5000))
        .show()?;

    Ok(())
}

fn is_user_in_wheel() -> bool {
    let username = match get_current_username() {
        Some(s) => s,
        None => {
            eprintln!("Can't get current user!");
            return false;
        }
    };

    let groups = match get_user_groups(&username, get_current_gid()) {
        Some(g) => g,
        None => {
            eprintln!("Couldn't get the list of groups the user is in.");
            return false;
        }
    };

    // check if in wheel
    for group in groups {
        if group.name() == OsStr::new("network") {
            return true;
        }
    }

    false
}
