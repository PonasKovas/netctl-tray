# netctl-tray
A lightweight netctl tray app with notifications written in Rust.

## Screenshots

![](https://i.imgur.com/5PavZiO.png) ![](https://i.imgur.com/mwWpkA4.png) ![](https://i.imgur.com/yghZ4Gt.png)

## Usage

To launch the tray app:
```
$ netctl-tray
```
You have to be in groups `wheel` and `network` for it to work properly.  
To add an user to them, use:
```
# usermod -a -G wheel,network <user>
```

## Compiling

This application needs to be compiled for netctl and netctl-auto separately.
For `netctl`:
```
cargo build --release
```
For `netctl-auto`:
```
cargo build --release --features "auto"
```

## Installation

This app is available on the AUR: [netctl-tray](https://aur.archlinux.org/packages/netctl-tray/) and [netctl-tray-auto](https://aur.archlinux.org/packages/netctl-tray-auto/)

## Contributing

All contributions are welcome!
