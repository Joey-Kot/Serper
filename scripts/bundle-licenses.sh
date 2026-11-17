#!/usr/bin/env bash
set -euo pipefail

# Bundle license texts for normal and build dependencies on every target.
# Usage: scripts/bundle-licenses.sh <output-directory>
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <output-directory>" >&2
  exit 2
fi
output_dir=$1
mkdir -p "$output_dir/THIRD_PARTY_LICENSES"
metadata_file=$(mktemp)
tree_file=$(mktemp)
rows_file=$(mktemp)
trap 'unlink "$metadata_file"; unlink "$tree_file"; unlink "$rows_file"' EXIT
cargo metadata --locked --format-version 1 > "$metadata_file"
cargo tree --locked --target all --edges normal,build --prefix none --format '{p}' > "$tree_file"
jq -r '.packages | sort_by(.name, .version)[] | select(.source != null) | [.name, .version, .manifest_path, (.license // "not specified"), (.repository // .homepage // "not specified")] | @tsv' "$metadata_file" > "$rows_file"

declare -A included=()
while read -r name version _; do
  included["$name ${version#v}"]=1
done < "$tree_file"

notice="$output_dir/THIRD_PARTY_NOTICES.txt"
printf '# Third-Party Notices\n\nLicense texts for the Rust dependencies used to build the CLI are included in THIRD_PARTY_LICENSES/.\nThe list includes dependencies for all supported platforms.\n\n' > "$notice"
while IFS=$'\t' read -r name version manifest license repository; do
  [[ ${included["$name $version"]+present} ]] || continue
  source_dir=${manifest%/*}
  dest_dir="$output_dir/THIRD_PARTY_LICENSES/$name-$version"
  mkdir -p "$dest_dir"
  found=0
  while IFS= read -r -d '' file; do
    cp "$file" "$dest_dir/"
    found=1
  done < <(find "$source_dir" -maxdepth 1 -type f \( -iname 'license*' -o -iname 'copying*' -o -iname 'notice*' -o -iname 'unlicense*' -o -iname 'copyright*' \) -print0)
  if [[ $found -eq 0 ]]; then
    echo "No license file found for $name $version ($source_dir)" >&2
    exit 1
  fi
  printf '## %s %s\n\n- License: %s\n- Source: %s\n- Texts: THIRD_PARTY_LICENSES/%s-%s/\n\n' "$name" "$version" "$license" "$repository" "$name" "$version" >> "$notice"
done < "$rows_file"
