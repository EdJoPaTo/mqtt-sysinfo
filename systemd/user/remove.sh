#!/usr/bin/env bash

systemctl --user disable --now "mqtt-sysinfo.service"

CONFIG_DIR=${XDG_CONFIG_DIRS:-"$HOME/.config"}
rm -f "$CONFIG_DIR/systemd/user/mqtt-sysinfo.service"
rm -f "$HOME/.local/bin/mqtt-sysinfo"

systemctl --user daemon-reload
