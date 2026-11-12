#!/bin/sh
# Copyright (C) 2026 ItsAlphaNeon
# SPDX-License-Identifier: GPL-3.0-or-later
# This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

# Generate the app bundle icon with macOS's built-in tools.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
assets="$root/crates/os-app/assets"
source="$assets/logo/ml-emblem.png"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
iconset="$work/lemonstudio.iconset"
mkdir "$iconset"
for points in 16 32 128 256 512; do
    sips -z "$points" "$points" "$source" --out "$iconset/icon_${points}x${points}.png" >/dev/null
    pixels=$((points * 2))
    sips -z "$pixels" "$pixels" "$source" --out "$iconset/icon_${points}x${points}@2x.png" >/dev/null
done
iconutil -c icns -o "$assets/lemonstudio.icns" "$iconset"
echo "$assets/lemonstudio.icns"
