#!/usr/bin/env bash
set -eu

dir=$(basename "$(pwd)")
if [ "$dir" == "systemd" ] || [ "$dir" == "user" ]; then
	echo "run from main directory like this: ./systemd/user/install.sh"
	exit 1
fi

# Create config/bin folders
CONFIG_DIR=${XDG_CONFIG_DIRS:-"$HOME/.config"}
mkdir -p "$CONFIG_DIR/systemd/user/" "$HOME/.local/bin"

nice cargo build --release --locked

# systemd
cp -v systemd/user/service "$CONFIG_DIR/systemd/user/mqtt-sysinfo.service"
systemctl --user daemon-reload

# stop, replace and start new version
systemctl --user stop "mqtt-sysinfo.service"
cp -v "target/release/mqtt-sysinfo" "$HOME/.local/bin"

systemctl --user enable --now "mqtt-sysinfo.service"
