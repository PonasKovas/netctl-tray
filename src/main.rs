use qt_widgets::{QSystemTrayIcon, QApplication, QMenu, QActionGroup, SlotOfActivationReason};
use qt_widgets::qt_core::{QTimer, QString, Slot, QByteArray};
use qt_widgets::cpp_utils::CppBox;
use qt_gui::{QIcon, QPixmap, QImage};
use std::process::Command;
use std::net::{TcpStream, SocketAddr};
use regex::RegexBuilder;
use std::time::Duration;
use std::fs;
use std::thread;
use shells::sh;

/// NoProfile - no profile is active
/// Good/Medium/Bad - connection strength
/// The bool - is there internet?
enum Status {
	NoProfile,
	Good(bool),
	Medium(bool),
	Bad(bool),
}

fn main() {
	// Check if started as root
	let as_root = match std::env::var("USER") {
		Ok(u) => { u=="root" },
		Err(_)=> false,
	};
	if !as_root {
		println!("Warning: tray started without root privileges! Some functions may not work!");
	}
	// Start another thread for communicating with netctl
	QApplication::init(|_app| {
		unsafe {
			let icons = [
				load_icon("/opt/netctl-tray/no_profile.svg"),
				load_icon("/opt/netctl-tray/good.svg"),
				load_icon("/opt/netctl-tray/medium.svg"),
				load_icon("/opt/netctl-tray/bad.svg"),
				load_icon("/opt/netctl-tray/good_no_internet.svg"),
				load_icon("/opt/netctl-tray/medium_no_internet.svg"),
				load_icon("/opt/netctl-tray/bad_no_internet.svg"),
			];
			// initiliaze tray
			let mut tray = QSystemTrayIcon::from_q_icon(
				icons[get_status_icon()].as_ref()
			);
			// Show the status notification on click of the tray
			let tray_click = SlotOfActivationReason::new(|reason| {
				let reason = reason.to_int();
				if reason == 3 || reason == 4 {
					thread::spawn(move || {
						// Left-click or middle-click
						// Find out the active profile
						let mut active_profile = "none".to_string();
						for (active, name) in get_profiles() {
							if active {
								active_profile = name;
								break;
							}
						}
						send_notification(&format!(
							"Profile: <b>{}</b>, Ping: <b>{} ms</b>, Quality: <b>{}/70</b>",
							active_profile,
							ping(),
							if active_profile == "none" { 0 } else {conn_strength(&active_profile) },
						), as_root);
					});
				}
			});
			tray.activated().connect(&tray_click);

			// Add the menu
			let mut menu = QMenu::new();
			tray.set_context_menu(menu.as_mut_ptr());
			// Add profiles submenu
			let profiles_submenu = menu.add_menu_q_string(
				QString::from_std_str("Profiles").as_mut_ref()
			);
			let mut profile_actions_group = QActionGroup::new(profiles_submenu);
			let group_ptr = profile_actions_group.as_mut_ptr();
			let click = Slot::new( || {
				set_profile( (*group_ptr.checked_action().text()).to_std_string() );
			});
			// Always update the profiles submenu before showing
			let mut ptr_profiles_submenu = profiles_submenu.as_mut_ref().unwrap();
			let generate_profiles_submenu = Slot::new(|| {
				ptr_profiles_submenu.clear();
				for (active, profile) in get_profiles() {
					if active {
						// Add the button with an icon
						let mut action = ptr_profiles_submenu.add_action_q_string(
							QString::from_std_str(&profile).as_mut_ref()
						);
						action.set_checkable(true);
						action.set_checked(true);
						action.set_action_group(profile_actions_group.as_mut_ptr());
						action.triggered().connect(&click);
					} else {
						// Add the button without the "active" icon
						let mut action = ptr_profiles_submenu.add_action_q_string(
							QString::from_std_str(&profile).as_mut_ref()
						);
						action.set_checkable(true);
						action.set_checked(false);
						action.set_action_group(profile_actions_group.as_mut_ptr());
						action.triggered().connect(&click);
					}
					
				}
			});
			profiles_submenu.about_to_show().connect( &generate_profiles_submenu );
			// Add button to exit
			let exit_app = Slot::new(|| {
				std::process::exit(0);
			});
			menu.add_action_q_icon_q_string(
				QIcon::from_q_string(
					QString::from_std_str("/opt/netctl-tray/exit.svg").as_mut_ref()
				).as_mut_ref(),
				QString::from_std_str("Exit").as_mut_ref()
			).triggered().connect(&exit_app);

			tray.show();

			// Make a function which will update the tray stuff when needed
			let update_tray = Slot::new(move || {
				// Update the tray icon based on the status of the connection
				tray.set_icon(
					icons[get_status_icon()].as_ref()
				);
			});
			let mut update_timer = QTimer::new_0a();
			// Call it every second
			update_timer.set_interval(1000);
			update_timer.timeout().connect(&update_tray);
			update_timer.start_0a();

			QApplication::exec()
		}
	})
}

/// Returns a path to an icon depending on the status of the wifi 
fn get_status_icon() -> usize {
	match get_status() {
		Status::NoProfile       => 0,
		Status::Good(true)      => 1,
		Status::Medium(true)    => 2,
		Status::Bad(true)       => 3,
		Status::Good(false)     => 4,
		Status::Medium(false)   => 5,
		Status::Bad(false)      => 6,
	}
}

fn get_status() -> Status {
	// Check if any profiles are active
	let active_profile = Command::new("netctl")
			.arg("list")
			.output()
			.expect("failed to run netctl").stdout;
	if !active_profile.contains(&42) { // An asterisk
		return Status::NoProfile;
	}
	
	// Check if there's internet
	let internet = match TcpStream::connect_timeout(&SocketAddr::from(([1, 1, 1, 1], 53)), Duration::from_millis(500)) {
		Ok(_) => true,
		Err(_) => false,
	};

	let active_profile = RegexBuilder::new(r"^\* (.+)$")
		.multi_line(true)
		.build().unwrap()
		.captures(std::str::from_utf8(&active_profile).unwrap())
		.expect("Couldn't parse netctl list output");

	let conn_strength = conn_strength(&active_profile[1]) as f32;

	// Finally return the status
	match (conn_strength/24f32).ceil() as u8 {
		3u8 => Status::Good(internet),
		2u8 => Status::Medium(internet),
		_ => Status::Bad(internet),
	}
}

fn get_profiles() -> Vec<(bool, String)> {
	let mut profiles = Vec::new();
	// Get the list of all profiles
	let raw_profiles = Command::new("netctl")
		.arg("list")
		.output()
		.expect("failed to run netctl").stdout;
	// Iterate through each line
	for line in raw_profiles.split(|c| *c == '\n' as u8) {
		if line.len() == 0 { continue; }
		// If the line starts with an asterisk, then the profile is active
		let active = line[0] == '*' as u8;
		let profile_name = std::str::from_utf8(&line[2..]).unwrap().to_string();
		profiles.push((active, profile_name));
	}

	profiles
}

fn set_profile(profile: String) {
	thread::spawn( move || {
		// If the profile is already active, turn it off, otherwise - turn it on
		let profiles = get_profiles();
		for (active, name) in profiles {
			if active && name==profile {
				// It's already active
				Command::new("netctl")
						.arg("stop")
						.arg(profile)
						.output()
						.expect("failed to run netctl");
				return;
			}
		}
		// It's not active, start it
		Command::new("netctl")
				.arg("switch-to")
				.arg(profile)
				.output()
				.expect("failed to run netctl");
		});
}

unsafe fn load_icon(path: &str) -> CppBox<QIcon> {
	QIcon::from_q_pixmap(
		QPixmap::from_image_1a(
			QImage::from_data_q_byte_array(
				QByteArray::from_slice(
					fs::read_to_string(path).unwrap().as_bytes()
				).as_ref()
			).as_ref()
		).as_ref()
	)
}

fn send_notification(message: &str, as_root: bool) {
	if as_root {
		let (_, display, _) = sh!("echo -n $(ls /tmp/.X11-unix/* | sed 's#/tmp/.X11-unix/X##' | head -n 1)");
		let (_, user, _) = sh!("echo -n $(who | grep '(:{})' | awk '{{print $1}}' | head -n 1)", display);
		let (_, uid, _) = sh!("echo -n $(id -u {})", user);

		sh!("su {} -c \"DISPLAY=:{} DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{}/bus notify-send 'netctl' '{}' -t 4000 -i network-wireless\"", user, display, uid, message);
	} else {
		sh!("notify-send 'netctl' '{}' -t 4000 -i network-wireless", message);
	}
	
}

fn ping() -> String {
	let (_, mut ping, _) = sh!("ping -qc1 google.com 2>&1 | awk -F'/' 'END{{ print (/^rtt/? $5:\"∞\") }}'");
	ping = ping.trim().to_string();
	match ping.parse::<f64>() {
		Ok(n) => n.ceil().to_string(),
		Err(_)=> ping
	}
}

fn conn_strength(profile: &str) -> u8 {
	// Get the interface the active profile is using
	let used_interface = Command::new("cat")
			.arg("/etc/netctl/".to_owned()+profile)
			.output()
			.expect(&format!("failed to read /etc/netctl/{}", profile)).stdout;
	let used_interface = RegexBuilder::new(r"^Interface\s*=\s*(.+)$")
		.multi_line(true)
		.case_insensitive(true)
		.build().unwrap()
		.captures(std::str::from_utf8(&used_interface).unwrap())
		.expect(&format!("Couldn't read the interface from /etc/netctl/{}", profile));

	// Check the strength of the connection
	let conn_strength = Command::new("iwconfig")
			.output()
			.expect("failed to run iwconfig").stdout;
	let conn_strength =
		RegexBuilder::new(&((&used_interface[1]).to_string() + r"(.|\n)+?Link Quality=([0-9]+)/70"))
		.case_insensitive(true)
		.build().unwrap()
		.captures(std::str::from_utf8(&conn_strength).unwrap())
		.expect(&format!("Failed to parse the output of iwconfig"));
	let conn_strength: u8 = (&conn_strength[2]).to_string().parse().unwrap();
	conn_strength
}