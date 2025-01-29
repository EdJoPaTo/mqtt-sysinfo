#!/usr/bin/env bash

sudo systemctl disable --now "mqtt-sysinfo.service"

sudo rm -f "/usr/local/lib/systemd/system/mqtt-sysinfo.service"
sudo rm -f "/usr/local/bin/mqtt-sysinfo"

sudo systemctl daemon-reload
