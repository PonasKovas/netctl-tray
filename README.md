# netctl-tray
A lightweight netctl tray app with notifications written in Rust.

## Screenshots

![](https://i.imgur.com/5PavZiO.png) ![](https://i.imgur.com/mwWpkA4.png) ![](https://i.imgur.com/yghZ4Gt.png)

## Usage

To launch the tray app:
```
$ netctl-tray
OR
$ netctl-auto-tray
```
Note: *launching the app as root is not safe*. But, `netctl-auto-tray` must call
`netctl-auto {list|switch-to}`, which require root. If using `netctl-auto-tray`,
add the following line to `/etc/sudoers` (or a `#include`/`#includedir` included
file) using `sudo visudo`. Be VERY CAREFUL editing `/etc/sudoers`: consult
`man sudoers` before doing so if unsure. Replace `<USER>` with the username (no
`<` or `>`).

```
<USER> ALL = (root) NOPASSWD: /usr/bin/netctl-auto list, /usr/bin/netctl-auto switch-to ?*
```

## Installation

[This app is available on the AUR](https://aur.archlinux.org/packages/netctl-tray/)
```
$ git clone https://aur.archlinux.org/netctl-tray.git
$ cd netctl-tray
$ makepkg -si
```

Non-Arch users can run `sudo ./install.sh` to install files under prefix `/`.

## Troubleshooting

If connect/disconnect notifications don't work for you, it's probably because you have multiple hooks set-up in `/etc/netctl/hooks/` incorrectly.
See [this section on arch wiki](https://wiki.archlinux.org/index.php/Netctl#Hooks_don't_work).

To fix it, edit all other hooks from something like this:
```sh
ExecUpPost="some command"
ExecDownPre="another command"
```
to something like this:
```sh
ExecUpPost="some command ; "$ExecUpPost
ExecDownPre="another command ; "$ExecDownPre
```

If connection strength can't be determined (`failed to read /etc/netctl/...` or
similar), ensure that the profile files in `/etc/netctl/` are readable by the
tray process. The easiest way to do this is to
`sudo chown root:<group> <profile>` where `<group>` is a group the user running
`netctl-{auto-}tray` is in, and then `sudo chmod g+r <profile>`.

## Contributing

All contributions are welcome!
