#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }
command -v node >/dev/null || { echo "node is required for JavaScript tests" >&2; exit 1; }
command -v wget >/dev/null || { echo "wget is required to repair the AppImage bundle" >&2; exit 1; }
test -x /usr/bin/readelf || { echo "binutils is required for the library license audit" >&2; exit 1; }
test -x /usr/bin/dpkg-query || { echo "this build script requires Ubuntu or Debian" >&2; exit 1; }

cargo test --locked --lib
node --test tests/*.test.cjs
node --check frontend/main.js
node --check frontend/window-size.js
git diff --check

export APPIMAGE_EXTRACT_AND_RUN=1
export LINUXDEPLOY_EXCLUDED_LIBRARIES='libwayland-client.so*'
cargo tauri build --bundles appimage -- --locked

shopt -s nullglob
images=(target/release/bundle/appimage/*.AppImage)
if (( ${#images[@]} != 1 )); then
  echo "Expected exactly one AppImage, found ${#images[@]}." >&2
  exit 1
fi

image=$(realpath "${images[0]}")
work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT
(
  cd "$work_dir"
  "$image" --appimage-extract >/dev/null
)
test -x "$work_dir/squashfs-root/AppRun"

if find "$work_dir/squashfs-root" -name 'libwayland-client.so*' -print -quit | grep -q .; then
  find "$work_dir/squashfs-root" -name 'libwayland-client.so*' -delete
fi

install -Dm644 packaging/linux/com.jim.ytdlprustygui.metainfo.xml \
  "$work_dir/squashfs-root/usr/share/metainfo/com.jim.ytdlprustygui.metainfo.xml"
node scripts/audit-appimage-licenses.mjs install "$work_dir/squashfs-root"
appimagetool="$work_dir/appimagetool.AppImage"
wget --https-only --secure-protocol=TLSv1_2 -O "$appimagetool" \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod 700 "$appimagetool"
ARCH=x86_64 "$appimagetool" --appimage-extract-and-run \
  "$work_dir/squashfs-root" "$work_dir/rebuilt.AppImage"
mv "$work_dir/rebuilt.AppImage" "$image"

verify_dir="$work_dir/verify"
mkdir "$verify_dir"
(
  cd "$verify_dir"
  "$image" --appimage-extract >/dev/null
)
test -x "$verify_dir/squashfs-root/AppRun"
if find "$verify_dir/squashfs-root" -name 'libwayland-client.so*' -print -quit | grep -q .; then
  echo 'Final AppImage still bundles libwayland-client.' >&2
  exit 1
fi
node scripts/audit-appimage-licenses.mjs audit "$verify_dir/squashfs-root"

candidate_dir=target/release/bundle/appimage/linux-candidate
mkdir -p "$candidate_dir"
cp "$image" "$candidate_dir/"
cp docs/LINUX_RELEASE.md "$candidate_dir/TESTING.md"
cp "$verify_dir/appimage-license-audit.txt" "$candidate_dir/"
{
  git rev-parse HEAD
  uname -m
  cat /etc/os-release
  rustc --version
  cargo tauri --version
} > "$candidate_dir/BUILD-INFO.txt"
(
  cd "$candidate_dir"
  sha256sum -- *.AppImage > SHA256SUMS
)
tar -czf target/release/bundle/appimage/linux-candidate.tar.gz \
  -C "$candidate_dir" .

echo "Linux candidate: $repo_root/target/release/bundle/appimage/linux-candidate.tar.gz"
