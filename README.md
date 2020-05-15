# netctl-tray
A lightweight netctl tray app with notifications written in Rust.

## Screenshots

![](https://i.imgur.com/5PavZiO.png) ![](https://i.imgur.com/mwWpkA4.png) ![](https://i.imgur.com/yghZ4Gt.png)

## Usage

To launch the tray app:
```
$ netctl-tray
```
Note: *launching the app as root is not safe*.

## Installation

Note: if you use `netctl-auto`, add `--features auto` to the cargo build command

```
$ cargo build --release # --features auto if netctl-auto is used
$ sudo ./install
```

## Troubleshooting

If you have a `netctl` profile named `None` you may run into issues.

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
`netctl-tray` is in, and then `sudo chmod g+r <profile>`.

`iwconfig` must also be installed.

## Contributing

All contributions are welcome!
