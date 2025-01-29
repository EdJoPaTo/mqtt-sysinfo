#!/usr/bin/env bash
set -eu

dir=$(basename "$(pwd)")
if [ "$dir" == "systemd" ] || [ "$dir" == "system" ]; then
	echo "run from main directory like this: ./systemd/system/install.sh"
	exit 1
fi

nice cargo build --release --locked

# systemd
function copyIntoLocal() {
	sudo mkdir -p "$(dirname "$2")"
	sed -e 's#/usr/#/usr/local/#' -e 's#/var/#/var/local/#' "$1" | sudo tee "$2" >/dev/null
}
copyIntoLocal systemd/system/service "/usr/local/lib/systemd/system/mqtt-sysinfo.service"
sudo systemctl daemon-reload

# stop, replace and start new version
sudo systemctl stop "mqtt-sysinfo.service"
sudo cp -v "target/release/mqtt-sysinfo" /usr/local/bin/

sudo systemctl enable --now "mqtt-sysinfo.service"
