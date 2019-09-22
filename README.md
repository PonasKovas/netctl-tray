# netctl-tray
A lightweight netctl tray app with notifications written in Rust.

## Screenshots

![](https://i.imgur.com/5PavZiO.png) ![](https://i.imgur.com/mwWpkA4.png) ![](https://i.imgur.com/yghZ4Gt.png)

## Usage

To launch the tray app:
```
# netctl-tray
```
Note: make sure to launch the app as *root*, otherwise some features, like profile changing, won't work.

## Installation

This app is available on the AUR
```
$ git clone https://aur.archlinux.org/netctl-tray.git
& cd netctl-tray
$ makepkg -si
```

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

## Contributing

All contributions are welcome!
