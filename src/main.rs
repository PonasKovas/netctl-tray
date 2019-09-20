use qt_widgets::{QSystemTrayIcon, QApplication, QMenu};
use qt_widgets::qt_core::{QTimer, QString, Slot};
use qt_gui::QIcon;
use std::process::Command;
use std::net::{TcpStream, SocketAddr};
use regex::RegexBuilder;

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
	// Start another thread for communicating with netctl
	QApplication::init(|_app| {
        unsafe {
            // initiliaze tray
            let mut tray = QSystemTrayIcon::from_q_icon(
            	QIcon::from_q_string(
            		QString::from_std_str(get_status_icon()).as_mut_ref()
            	).as_mut_ref()
            );

            // Add a menu with an option to exit
            let mut menu = QMenu::new();
            let exit_app = Slot::new(|| {
            	std::process::exit(0);
            });
            menu.add_action_q_icon_q_string(
            	QIcon::from_theme_1a(
            		QString::from_std_str("/home/mykolas/rust/netctl-tray/assets/exit.svg").as_mut_ref()
            	).as_mut_ref(),
            	QString::from_std_str("Exit").as_mut_ref()
            ).triggered().connect(&exit_app);
            tray.set_context_menu(menu.as_mut_ptr());

            tray.show();

            // Make a function which will update the tray stuff when needed
            let update_tray = Slot::new(move || {
            	// Update the tray icon based on the status of the connection
            	tray.set_icon(
            		QIcon::from_q_string(
	            		QString::from_std_str(get_status_icon()).as_mut_ref()
	            	).as_mut_ref()
            	);
            });
            let mut update_timer = QTimer::new_0a();
            update_timer.set_interval(1000);
            update_timer.timeout().connect(&update_tray);
            update_timer.start_0a();

            QApplication::exec()
        }
    })
}

/// Returns a path to an icon depending on the status of the wifi 
fn get_status_icon() -> String {
	match get_status() {
		Status::NoProfile	 => "/home/mykolas/rust/netctl-tray/assets/no_profile.svg".to_string(),
		Status::Good(true)	 => "/home/mykolas/rust/netctl-tray/assets/good.svg".to_string(),
		Status::Medium(true) => "/home/mykolas/rust/netctl-tray/assets/medium.svg".to_string(),
		Status::Bad(true)	 => "/home/mykolas/rust/netctl-tray/assets/bad.svg".to_string(),
		Status::Good(false)	 => "/home/mykolas/rust/netctl-tray/assets/good_no_internet.svg".to_string(),
		Status::Medium(false)=> "/home/mykolas/rust/netctl-tray/assets/medium_no_internet.svg".to_string(),
		Status::Bad(false)	 => "/home/mykolas/rust/netctl-tray/assets/bad_no_internet.svg".to_string(),
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
	let internet = match TcpStream::connect(&SocketAddr::from(([1, 1, 1, 1], 53))) {
		Ok(_) => true,
		Err(_) => false,
	};

	let active_profile = RegexBuilder::new(r"^\* (.+)$")
		.multi_line(true)
		.build().unwrap()
		.captures(std::str::from_utf8(&active_profile).unwrap())
		.expect("Couldn't parse netctl list output");

	// Get the interface the active profile is using
	let used_interface = Command::new("cat")
			.arg("/etc/netctl/".to_owned()+&active_profile[1])
            .output()
            .expect(&format!("failed to read /etc/netctl/{}", &active_profile[1])).stdout;
    let used_interface = RegexBuilder::new(r"^Interface\s*=\s*(.+)$")
		.multi_line(true)
		.case_insensitive(true)
		.build().unwrap()
		.captures(std::str::from_utf8(&used_interface).unwrap())
		.expect(&format!("Couldn't read the interface from /etc/netctl/{}", &active_profile[1]));

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
	let conn_strength: f32 = (&conn_strength[2]).to_string().parse().unwrap();

	// Finally return the status
	match (conn_strength/24f32).ceil() as u8 {
		3u8 => Status::Good(internet),
		2u8 => Status::Medium(internet),
		_ => Status::Bad(internet),
	}
}