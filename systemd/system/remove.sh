#!/usr/bin/env bash

sudo systemctl disable --now "mqtt-hostname-online.service"

sudo rm -f "/usr/local/lib/systemd/system/mqtt-hostname-online.service"
sudo rm -f "/usr/local/bin/mqtt-hostname-online"

sudo systemctl daemon-reload
