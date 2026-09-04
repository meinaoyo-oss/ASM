#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(cargo metadata --no-deps --format-version 1 --manifest-path "$project_root/Cargo.toml" | jq -r '.packages[0].version')"
dist_dir="$project_root/dist"
stage_root="$(mktemp -d)"
trap 'rm -rf "$stage_root"' EXIT

build_package() {
    local target="$1"
    local platform="$2"
    local executable="$3"
    local archive_kind="$4"
    local package_name="electronics-manufacturing-mcp-v${version}-${platform}"
    local stage="$stage_root/$package_name"

    cargo build --manifest-path "$project_root/Cargo.toml" --release --locked --target "$target"
    mkdir -p "$stage/bin" "$stage/config"
    cp "$project_root/target/$target/release/$executable" "$stage/bin/$executable"
    cp "$project_root/config/default.toml" "$stage/config/default.toml"
    cp "$project_root/packaging/manifest.${platform}.json" "$stage/manifest.json"
    cp -R "$project_root/skills" "$stage/skills"
    cp "$project_root/LICENSE" "$project_root/THIRD_PARTY_NOTICES.md" "$project_root/README.md" "$stage/"
    (
        cd "$stage"
        find . -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS.txt
    )

    if [[ "$archive_kind" == "zip" ]]; then
        (
            cd "$stage_root"
            zip -qr "$stage_root/$package_name.zip" "$package_name"
        )
        mv -f "$stage_root/$package_name.zip" "$dist_dir/$package_name.zip"
    else
        tar -C "$stage_root" -czf "$stage_root/$package_name.tar.gz" "$package_name"
        mv -f "$stage_root/$package_name.tar.gz" "$dist_dir/$package_name.tar.gz"
    fi
}

mkdir -p "$dist_dir"
build_package "x86_64-unknown-linux-musl" "linux-x86_64" "electronics-manufacturing-mcp" "tar"
build_package "x86_64-pc-windows-gnu" "windows-x86_64" "electronics-manufacturing-mcp.exe" "zip"
(
    cd "$dist_dir"
    sha256sum ./*.tar.gz ./*.zip > SHA256SUMS.txt
)
