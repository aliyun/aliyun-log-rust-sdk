#!/usr/bin/env bash
# Install the pinned runtime rather than whichever GraalPy the build image ships.
set -euo pipefail
archive=$(mktemp)
trap 'rm -f "$archive"' EXIT
curl --fail --location --retry 3 "$1" --output "$archive"
printf '%s  %s\n' "$2" "$archive" | sha256sum --check -
mkdir -p /opt/sls-graalpy
tar -xzf "$archive" --strip-components=1 -C /opt/sls-graalpy
/opt/sls-graalpy/bin/graalpy --version
