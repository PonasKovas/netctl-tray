use qt_widgets::{QSystemTrayIcon, QApplication, QMenu, QActionGroup, SlotOfActivationReason};
use qt_widgets::qt_core::{QTimer, QString, Slot};
use qt_gui::QIcon;
use std::thread;

mod utils;

fn main() {
	// Check if started as root
	let as_root = match std::env::var("USER") {
		Ok(u) => { u=="root" },
		Err(_)=> false,
	};
	if as_root {
		println!("Warning: tray started as root! Aborting.");
      return;
	}
	// Start another thread for communicating with netctl
	QApplication::init(|_app| {
		unsafe {
			let icons = [
				utils::load_icon("/usr/share/netctl-tray/assets/no_profile.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/good.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/medium.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/bad.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/no_signal.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/good_no_internet.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/medium_no_internet.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/bad_no_internet.svg"),
				utils::load_icon("/usr/share/netctl-tray/assets/no_signal_no_internet.svg"),
			];
			// initiliaze tray
			let mut tray = QSystemTrayIcon::from_q_icon(
				icons[utils::get_status_icon()].as_ref()
			);
			// Show the status notification on click of the tray
			let tray_click = SlotOfActivationReason::new(|reason| {
				let reason = reason.to_int();
				if reason == 3 || reason == 4 {
					thread::spawn(move || {
						// Left-click or middle-click
						// Find out the active profile
						let mut active_profile = "None".to_string();
						for (active, name) in utils::get_profiles() {
							if active {
								active_profile = name;
								break;
							}
						}
						utils::send_notification(&format!(
							"Profile: <b>{}</b>, Ping: <b>{} ms</b>, Quality: <b>{}/70</b>",
							active_profile,
							utils::get_rtt_str(),
							if active_profile == "None" { 0 } else { utils::conn_strength(&active_profile) },
						));
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
				utils::set_profile( (*group_ptr.checked_action().text()).to_std_string() );
			});
			// Always update the profiles submenu before showing
			let mut ptr_profiles_submenu = profiles_submenu.as_mut_ref().unwrap();
			let generate_profiles_submenu = Slot::new(|| {
				ptr_profiles_submenu.clear();
				for (active, profile) in utils::get_profiles() {
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
			menu.about_to_show().connect( &generate_profiles_submenu );
			// Add button to exit
			let exit_app = Slot::new(|| {
				std::process::exit(0);
			});
			menu.add_action_q_icon_q_string(
				QIcon::from_q_string(
					QString::from_std_str("/usr/share/netctl-tray/assets/exit.svg").as_mut_ref()
				).as_mut_ref(),
				QString::from_std_str("Exit").as_mut_ref()
			).triggered().connect(&exit_app);

			tray.show();

			// Make a function which will update the tray stuff when needed
			let update_tray = Slot::new(move || {
				// Update the tray icon based on the status of the connection
				tray.set_icon(
					icons[utils::get_status_icon()].as_ref()
				);
			});
			let mut update_timer = QTimer::new_0a();
			// Call it every 2 seconds
			update_timer.set_interval(2000);
			update_timer.timeout().connect(&update_tray);
			update_timer.start_0a();

			QApplication::exec()
		}
	})
}
