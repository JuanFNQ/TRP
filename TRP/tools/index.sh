#!/usr/bin/env bash
# index.sh: writes docs/index.md, a map of every document and every source file with one line on
# what it holds. Run it after adding, moving or renaming a file:
#
#   bash tools/index.sh            rewrites docs/index.md
#   bash tools/index.sh --check    fails if docs/index.md is not what it would write
#
# A source file's line is the first sentence of its `//!` comment, so keep that sentence plain.
set -euo pipefail
export LC_ALL=C
cd "$(dirname "${BASH_SOURCE[0]}")/.."
out=docs/index.md
new="$(mktemp)"
trap 'rm -f "$new"' EXIT

# The first paragraph under a document's title, cut to one line.
about() {
  awk '{ sub(/\r$/, "") } /^# / && !seen { seen = 1; next } seen && NF && !/^[#|`>-]/ { end = match($0, /\. /); line = end ? substr($0, 1, end) : $0; print substr(line, 1, 220) (length(line) > 220 ? "..." : ""); exit }' "$1"
}

{
  echo "# Index"
  echo
  echo "Every document and every source file, with one line on what it holds. This file is generated: run \`bash tools/index.sh\` after adding, moving or renaming a file, and do not edit it by hand."
  echo
  echo "To find where something lives, search this file for a word (\`grep -i \"quick bar\" docs/index.md\`) and open the file it names. To read one section of a long spec, use \`bash tools/section.sh <spec> \"<heading>\"\`."
  echo
  echo "## Documents"
  echo
  for f in ../README.md ../AGENTS.md ../CHANGELOG.md docs/*.md; do
    if [ -f "$f" ] && [ "$f" != "$out" ]; then
      printf -- '- `%s`: %s\n' "${f#../}" "$(about "$f")"
    fi
  done
  echo
  echo "## Specs: how the game behaves (\`docs/spec/\`)"
  echo
  for f in docs/spec/*.md; do
    printf -- '- `%s` (%d KB): %s\n' "$(basename "$f")" "$(( ($(wc -c < "$f") + 512) / 1024 ))" "$(about "$f")"
  done
  echo
  echo "## Source"
  # A folder's files together, its own `mod.rs`, `main.rs` or `lib.rs` first.
  find crates -name '*.rs' -not -path '*/target/*' \
    | awk -F/ '{ name = $NF; dir = $0; sub(/\/[^\/]*$/, "", dir); print dir "\t" (name ~ /^(mod|main|lib)\.rs$/ ? 0 : 1) "\t" name }' \
    | sort -t "$(printf '\t')" -k1,1 -k2,2n -k3,3 \
    | awk -F'\t' '{ print $1 "/" $3 }' \
    | awk '
    {
      file = $0; lines = 0; doc = ""; open = 1
      while ((getline line < file) > 0) {
        lines++
        sub(/\r$/, "", line)
        if (open) {
          if (line ~ /^\/\/! ?[^ ]/) { sub(/^\/\/! ?/, "", line); doc = doc (doc == "" ? "" : " ") line }
          else if (doc != "" || line !~ /^(\/\/!|#!\[|$)/) open = 0
        }
      }
      close(file)
      end = match(doc, /\.( |$)/)
      if (end) doc = substr(doc, 1, end)
      if (length(doc) > 200) doc = substr(doc, 1, 197) "..."
      dir = file; sub(/\/[^\/]*$/, "", dir)
      name = file; sub(/^.*\//, "", name)
      if (dir != last) { printf "\n### %s\n\n", dir; last = dir }
      printf "- `%s` (%d): %s\n", name, lines, doc
    }'
} > "$new"

if [ "${1:-}" = "--check" ]; then
  if ! diff -q "$new" "$out" > /dev/null 2>&1; then
    echo "docs/index.md is out of date: run bash tools/index.sh" >&2
    exit 1
  fi
  exit 0
fi
cp "$new" "$out"
echo "docs/index.md: $(wc -l < "$out") lines"
