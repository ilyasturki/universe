#!/bin/sh
cd "$(dirname "$0")" || exit 1
exec nice -n 15 chromium --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size=1200,630 --virtual-time-budget=3000 --allow-file-access-from-files \
    --screenshot="$PWD/../public/og.png" "file://$PWD/card.html"
