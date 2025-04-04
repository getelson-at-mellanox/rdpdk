#!/bin/sh

# Required environment variables:
# DPDK_SOURCE_ROOT - DPDK sources base directory
# DPDK_BUILD_ROOT - DPDK build directory
# RDPDK_ROOT - Rust DPDK source directory

if test "x$dbg" != 'x'; then
  set -x
fi

if test "x$DPDK_SOURCE_ROOT" = 'x'; then
  echo "DPDK_SOURCE_ROOT was not defined"
  exit 255
fi

if test "x$DPDK_BUILD_ROOT" = 'x'; then
  echo "DPDK_BUILD_ROOT was not defined"
  exit 255
fi

if test "x$RDPDK_ROOT" = 'x'; then
  echo "RDPDK_ROOT was not defined"
  exit 255
fi

if test -d "$RDPDK_ROOT/raw_api"; then
  rm -rf "$RDPDK_ROOT/raw_api"
fi

dpdk_raw="$RDPDK_ROOT/lib/dpdk_raw"

rm -rf "$dpdk_raw" > /dev/null 2>&1
mkdir -p "$dpdk_raw"
if ! test -d "$dpdk_raw"; then
  echo "cannot create \"$dpdk_raw\""
  exit 255
fi

cat > "$dpdk_raw/dpdk_raw.rs" <<EOF
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]

EOF

for file in $RDPDK_ROOT/buildtools/rust.d/*; do
  /bin/echo -n "Build RUST API for $(basename $file) ... "
  out_dir="$dpdk_raw" sh -x $file
  if test "$?" -eq 0; then
    echo "Ok"
    echo "pub mod $(basename $file);" >> "$dpdk_raw/dpdk_raw.rs"
  else
    echo "Failed"
    exit 255
  fi
done

cd "$RDPDK_ROOT"
cargo fix --allow-no-vcs --allow-dirty --allow-staged --lib
cd -
