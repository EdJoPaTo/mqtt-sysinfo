#!/usr/bin/env bash
set -e

# build
nice cargo build --release

# systemd stuff
sudo mkdir -p /usr/local/lib/systemd/system/
sudo cp -uv ./*.service /usr/local/lib/systemd/system/
sudo systemctl daemon-reload

# copy to system
sudo systemctl stop mqtt-hostname-online.service
sudo cp -v target/release/mqtt-hostname-online /usr/local/bin/

# start
sudo systemctl start mqtt-hostname-online.service
