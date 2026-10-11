#!/usr/bin/env bash
set -euo pipefail

version=$1
arch=$2
asset="update_dns_record-${version}-linux-${arch}.tar.gz"
case "$arch" in
  amd64) machine='Advanced Micro Devices X86-64' ;;
  arm64) machine='AArch64' ;;
  *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
esac

# Verify the matching checksum even when the other architecture is not downloaded.
checksum=$(awk -v file="$asset" '$2 == file { print }' SHA256SUMS)
test -n "$checksum"
printf '%s\n' "$checksum" | sha256sum --check --strict
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
test "$(tar -tzf "$asset")" = update_dns_record
tar -xzf "$asset" -C "$work"
test -x "$work/update_dns_record"
readelf -h "$work/update_dns_record" | grep -F "$machine"
test "$("$work/update_dns_record" --version)" = "update_dns_record $version"
"$work/update_dns_record" --help > /dev/null
ldd "$work/update_dns_record"
