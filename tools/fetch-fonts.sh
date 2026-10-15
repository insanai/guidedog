#!/bin/sh
# Fetches the Noto Serif CJK faces the Chinese, Japanese, and Korean PDF manuals use into
# docs/shared/fonts, and checks each against its pinned SHA-256. They are 25 MB each, so
# they are not kept in the repository. A file already present with the right checksum is
# kept; a wrong one is replaced. Usage: sh tools/fetch-fonts.sh [folder]
set -eu

dir=${1:-"$(dirname "$0")/../docs/shared/fonts"}
base=https://raw.githubusercontent.com/notofonts/noto-cjk/Serif2.003/Serif/OTF

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
    else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

fetch() {
    path=$1 sum=$2 file=$dir/${1##*/}
    if [ -f "$file" ] && [ "$(sha256 "$file")" = "$sum" ]; then return; fi
    echo "fetching ${file##*/}"
    curl -sSfL --retry 3 -o "$file.part" "$base/$path"
    got=$(sha256 "$file.part")
    if [ "$got" != "$sum" ]; then
        rm -f "$file.part"
        echo "error: ${file##*/} has SHA-256 $got, expected $sum" >&2
        echo "hint: the upstream file changed; check docs/shared/fonts/PROVENANCE.txt" >&2
        exit 1
    fi
    mv "$file.part" "$file"
}

mkdir -p "$dir"
fetch Japanese/NotoSerifCJKjp-Regular.otf \
    d9854c7a8ef170b5a7932558856fd64eb8de0b007cd823fed6f9f514ad2803d3
fetch Japanese/NotoSerifCJKjp-Bold.otf \
    861a2b2c0e24b23745c262be8c3fdef63f12628f0492fb120ee51aa55c503af8
fetch Korean/NotoSerifCJKkr-Regular.otf \
    77b4b741f864d27f15e90f275b17106dde90b2ad28f82bab72dc95805db5fb42
fetch Korean/NotoSerifCJKkr-Bold.otf \
    10cc03741178ad6d2747df8497d911e34b167d0474a826fb9d866c402cbe3d8f
fetch SimplifiedChinese/NotoSerifCJKsc-Regular.otf \
    2a2eae2628df83556c54018c41e20fa532c1b862c5256ae8b3f23feb918d12ca
fetch SimplifiedChinese/NotoSerifCJKsc-Bold.otf \
    8af07d4b6c2e82bcc72a30e066eaf295f11b9424f4aad2eaa9fe0e9c3b38fc73
