#!/bin/sh
set -eu
if [ "$#" -ne 1 ]; then
    echo 'Usage: ROLLBACK.sh /absolute/path/to/target-copy.ps1' >&2
    exit 2
fi
case "$1" in /*|?:/*) ;; *) echo 'Target copy must be an absolute path' >&2; exit 2;; esac
here=$(CDPATH= cd "$(dirname "$0")" && pwd)
cp "$here/baseline.ps1" "$1"
printf 'Restored pristine comparator: %s\n' "$1"
