#![feature(exclusive_range_pattern)]

mod state;

use notify_rust::{Notification, Timeout};
use qt_gui::QIcon;
use qt_widgets::cpp_utils::{CppBox, MutPtr};
use qt_widgets::qt_core::{QString, QTimer, Slot};
use qt_widgets::{QActionGroup, QApplication, QMenu, QSystemTrayIcon, SlotOfActivationReason};
use state::{inotify_watch, scan_profiles, update_state, State};
use std::ffi::OsStr;
use std::net::SocketAddr;
use std::process::Command;
use std::sync::{Arc, Mutex};
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

    // Then make sure the user running this is in the group `wheel` and `network`
    let (in_wheel, in_network) = is_user_in_wheel_and_network();
    if !in_wheel {
        eprintln!("Warning! You are not in group 'wheel', netctl-tray might not work or work only partially.");
    }
    if !in_network {
        eprintln!("Warning! You are not in group 'network', netctl-tray might not work or work only partially.");
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

    // We will need mutable access to state from multiple closures
    // And, since this application is singlethreaded, we can safely
    // use a mutable pointer.
    let state_ptr: *mut State = &mut state;

    // initialize QT application
    QApplication::init(|_app| {
        unsafe {
            // initiliaze tray
            let mut tray = QSystemTrayIcon::from_q_icon(get_status_icon(&mut state).as_ref());

            // Show the status notification on click of the tray icon
            let tray_click = SlotOfActivationReason::new(|reason| {
                let reason = reason.to_int();
                // Left-click or middle-click
                if reason == 3 || reason == 4 {
                    if let Err(e) = Notification::new()
                        .summary("netctl")
                        .body(&format!(
                            "Profile: <b>{}</b>, Ping: <b>{} ms</b>, Quality: <b>{}/70</b>",
                            (*state_ptr)
                                .active_profile
                                .as_ref()
                                .unwrap_or(&"<i>{none}</i>".to_string()),
                            if (*state_ptr).ping == f32::INFINITY {
                                "∞".to_string()
                            } else {
                                (*state_ptr).ping.round().to_string()
                            },
                            (*state_ptr).link_quality
                        ))
                        .icon("network-wireless")
                        .timeout(Timeout::Milliseconds(5000))
                        .show()
                    {
                        eprintln!("Error sending desktop notification: {:?}", e);
                    }
                }
            });
            tray.activated().connect(&tray_click);

            // Add the menu
            let mut menu = QMenu::new();
            tray.set_context_menu(menu.as_mut_ptr());

            // Add profiles submenu
            let profiles_submenu =
                menu.add_menu_q_string(QString::from_std_str("Profiles").as_mut_ref());
            let mut profile_actions_group = QActionGroup::new(profiles_submenu);
            let group_ptr = profile_actions_group.as_mut_ptr();
            let click = Slot::new(|| {
                #[cfg(not(feature = "auto"))]
                {
                    if let Some(current_active_profile) = &(*state_ptr).active_profile {
                        // stop the old profile
                        if let Err(e) = Command::new("netctl")
                            .arg("stop")
                            .arg(current_active_profile)
                            .spawn()
                        {
                            eprintln!("Couldn't run netctl stop command: {:?}", e);
                        }
                    }
                    // start the new profile
                    if let Err(e) = Command::new("netctl")
                        .arg("start")
                        .arg((*group_ptr).checked_action().text().to_std_string())
                        .spawn()
                    {
                        eprintln!("Couldn't run netctl start command: {:?}", e);
                    }
                }
                #[cfg(feature = "auto")]
                {
                    // switch to the new profile
                    if let Err(e) = Command::new("netctl-auto")
                        .arg("switch-to")
                        .arg((*group_ptr).checked_action().text().to_std_string())
                        .spawn()
                    {
                        eprintln!("Couldn't run netctl-auto switch-to command: {:?}", e);
                    }
                }
            });

            // Generate the menu when it needs to be shown
            let generate_profiles_submenu = Slot::new(|| {
                gen_profile_submenu(
                    state_ptr,
                    profiles_submenu,
                    &mut profile_actions_group,
                    &click,
                );
            });
            profiles_submenu
                .about_to_show()
                .connect(&generate_profiles_submenu);

            // Add button to exit
            let exit_app = Slot::new(|| {
                std::process::exit(0);
            });
            menu.add_action_q_icon_q_string(
                QIcon::from_q_string(
                    QString::from_std_str("/usr/share/netctl-tray/exit.svg").as_mut_ref(),
                )
                .as_mut_ref(),
                QString::from_std_str("Exit").as_mut_ref(),
            )
            .triggered()
            .connect(&exit_app);

            tray.show();

            // Update tray state every X seconds
            let update_state = Slot::new(|| {
                let old_active_profile = (*state_ptr).active_profile.clone();
                if let Err(e) = update_state(&mut (*state_ptr), &args) {
                    eprintln!("Can't update tray state: {:?}", e);
                }
                if !args.disable_notifications {
                    if let Err(e) = profile_notification(&mut (*state_ptr), old_active_profile) {
                        eprintln!("Error sending desktop notification: {:?}", e);
                    }
                }

                // Update the tray icon based on the new state
                tray.set_icon(get_status_icon(&mut (*state_ptr)).as_ref());
            });
            let mut update_timer = QTimer::new_0a();
            // Call it every second
            update_timer.set_interval((args.interval * 1000.0) as i32);
            update_timer.timeout().connect(&update_state);
            update_timer.start_0a();

            QApplication::exec()
        }
    });
}

unsafe fn gen_profile_submenu(
    state_ptr: *mut State,
    mut profiles_submenu: MutPtr<QMenu>,
    profile_actions_group: &mut CppBox<QActionGroup>,
    click: &Slot,
) {
    profiles_submenu.clear();
    for profile in &(*(*state_ptr).all_profiles.lock().unwrap()) {
        let mut item =
            profiles_submenu.add_action_q_string(QString::from_std_str(profile).as_mut_ref());
        item.set_checkable(true);
        item.set_checked(false);
        if let Some(active_profile) = &(*state_ptr).active_profile {
            if active_profile == profile {
                item.set_checked(true);
            }
        }
        item.set_action_group(profile_actions_group.as_mut_ptr());
        item.triggered().connect(click);
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

fn is_user_in_wheel_and_network() -> (bool, bool) {
    let username = match get_current_username() {
        Some(s) => s,
        None => {
            eprintln!("Can't get current user!");
            return (false, false);
        }
    };

    let groups = match get_user_groups(&username, get_current_gid()) {
        Some(g) => g,
        None => {
            eprintln!("Couldn't get the list of groups the user is in.");
            return (false, false);
        }
    };

    let mut in_wheel = false;
    let mut in_network = false;
    for group in groups {
        if group.name() == OsStr::new("network") {
            in_network = true;
        } else if group.name() == OsStr::new("wheel") {
            in_wheel = true;
        }
    }

    (in_wheel, in_network)
}

fn get_status_icon(state: &mut State) -> CppBox<QIcon> {
    let icon_path = if state.active_profile.is_none() {
        // no active profile
        "/usr/share/netctl-tray/no_profile.svg"
    } else {
        if state.ping == f32::INFINITY {
            // no internet
            match state.link_quality {
                0 => "/usr/share/netctl-tray/no_signal_no_internet.svg",
                1..23 => "/usr/share/netctl-tray/bad_no_internet.svg",
                23..47 => "/usr/share/netctl-tray/medium_no_internet.svg",
                _ => "/usr/share/netctl-tray/good_no_internet.svg",
            }
        } else {
            match state.link_quality {
                0 => "/usr/share/netctl-tray/no_signal.svg",
                1..23 => "/usr/share/netctl-tray/bad.svg",
                23..47 => "/usr/share/netctl-tray/medium.svg",
                _ => "/usr/share/netctl-tray/good.svg",
            }
        }
    };
    unsafe { QIcon::from_q_string(QString::from_std_str(&icon_path).as_mut_ref()) }
}
