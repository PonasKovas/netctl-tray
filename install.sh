# binary
install -Dm755 target/release/netctl-tray "/usr/bin/netctl-tray"
# resources
install -d "/usr/share/netctl-tray/"
# list and switch-to scripts
install -Dm755 scripts/netctl-list "/usr/share/netctl-tray/netctl-list"
install -Dm755 scripts/netctl-switch-to "/usr/share/netctl-tray/netctl-switch-to"
install -Dm755 scripts/netctl-auto-list "/usr/share/netctl-tray/netctl-auto-list"
install -Dm755 scripts/netctl-auto-switch-to "/usr/share/netctl-tray/netctl-auto-switch-to"
# hooks
install -Dm755 scripts/netctltray "/etc/netctl/hooks/netctltray"
install -Dm755 scripts/connect "/usr/share/netctl-tray/connect"
install -Dm755 scripts/disconnect "/usr/share/netctl-tray/disconnect"
# svg assets
install -d "/usr/share/netctl-tray/assets/"
install -Dm644 assets/* "/usr/share/netctl-tray/assets/"
# polkit
# polkit >= 0.106
install -dm750 "/usr/share/polkit-1/rules.d/"
install -Dm644 scripts/netctl-tray.rules "/usr/share/polkit-1/rules.d/netctl-tray.rules"
# polkit <= 0.105
install -Dm750 scripts/netctl-tray.policy "/usr/share/polkit-1/actions/netctl-tray.policy"
