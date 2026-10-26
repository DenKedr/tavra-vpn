#!/usr/bin/env bash
set -euo pipefail
for deb in dist/*.deb; do
 dpkg-deb --info "$deb"
 dpkg-deb --contents "$deb"
 tmp=$(mktemp -d)
 dpkg-deb --control "$deb" "$tmp"
 for script in postinst prerm postrm; do sh -n "$tmp/$script"; done
 rm -rf "$tmp"
done
# Optional install/remove smoke on throwaway containers only.
if [[ ${1:-} == --containers ]]; then
 for ubuntu in 22.04 24.04 26.04; do
  docker run --rm -v "$PWD/dist:/packages:ro" "ubuntu:$ubuntu" bash -euc '
   apt-get update -qq
   apt-get install -y --no-install-recommends adduser libgcc-s1 systemd
   dpkg -i /packages/*.deb
   for component in control node; do
    test -x /opt/tavra/bin/tavra-$component
    systemd-analyze verify /lib/systemd/system/tavra-$component.service
    /opt/tavra/bin/tavra-$component >/tmp/tavra-start.log 2>&1 && exit 1
   done
   dpkg --configure -a
   dpkg --purge tavra-control tavra-node
   test ! -e /opt/tavra/bin/tavra-control
   test ! -e /opt/tavra/bin/tavra-node
  '
 done
fi
