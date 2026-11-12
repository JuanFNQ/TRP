#!/usr/bin/env bash
# section.sh: TRP's documents read a section at a time. A spec's lines are whole paragraphs and
# the longest specs run to hundreds of KB, so read a section and never the whole file.
#
# section.sh <doc>                     the document's headings: line range, size, heading
# section.sh <doc> --leads             the same, and under each heading the paragraphs that open in bold
# section.sh <doc> <heading> [--all]   one section (a case-insensitive regex on the heading), to the next
#                                      heading of its level or higher; over 40 KB only its first 20 lines
#                                      unless --all
# section.sh --find <regex> [doc]      every line of the documents (or of one) that matches, case
#                                      ignored: the document, the line, the heading it is under and
#                                      the words around the match, never the whole paragraph
#
#   <doc> is a path, or a name looked up as TRP/docs/<name>.md, then TRP/docs/spec/<name>.md, then
#   <name>.md at the top. Run from anywhere in the repository, with Git's bash on Windows.
#   To find a source file, search docs/index.md (written by tools/index.sh).
set -euo pipefail
# Sizes in bytes and the same order on every machine.
export LC_ALL=C

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
trp="$root/TRP"

# ==== --find: a word in the documents ====
if [ "${1:-}" = "--find" ]; then
  pat="${2:-}"
  if [ -z "$pat" ]; then echo "usage: section.sh --find <regex> [doc]" >&2; exit 2; fi
  if [ -n "${3:-}" ]; then
    files=()
    for c in "$3" "$root/$3" "$trp/docs/$3.md" "$trp/docs/spec/$3.md" "$trp/docs/$3" "$root/$3.md" "$trp/$3"; do
      if [ -f "$c" ]; then files=("$c"); break; fi
    done
    if [ ${#files[@]} = 0 ]; then echo "no such document: $3" >&2; exit 1; fi
  else
    files=()
    while IFS= read -r f; do files+=("$f"); done < <(find "$trp/docs" "$trp/README.md" "$root/PATCH_NOTES.md" -name '*.md' 2>/dev/null | sort)
  fi
  awk -v pat="$pat" -v root="$root/" -v max=60 '
    FNR == 1 { h = ""; fence = 0; name = FILENAME; if (index(name, root) == 1) name = substr(name, length(root) + 1) }
    { sub(/\r$/, "") }
    /^```/ { fence = !fence }
    !fence && /^#+ / { h = $0 }
    {
      low = tolower($0)
      if (!match(low, tolower(pat))) next
      n++
      if (n > max) next
      from = RSTART - 80; if (from < 1) from = 1
      snip = substr($0, from, RLENGTH + 160)
      printf "%s:%d  [%s]\n    %s%s%s\n", name, FNR, substr(h, 1, 80), (from > 1 ? "..." : ""), snip, (from + RLENGTH + 160 <= length($0) ? "..." : "")
    }
    END {
      if (n == 0) { printf "nothing matches /%s/\n", pat > "/dev/stderr"; exit 1 }
      if (n > max) printf "== %d lines match; the first %d shown: narrow the pattern, or name a document ==\n", n, max
      else printf "== %d lines match. section.sh <doc> \"<heading>\" prints a section; or Read a few lines around a hit ==\n", n
    }
  ' "${files[@]}"
  exit 0
fi

# ==== a document or a source file ====
doc="${1:-}"
pat="${2:-}"
all=0
leads=0
for a in "${@:2}"; do
  case "$a" in
    --all) all=1; [ "$pat" = "--all" ] && pat="" ;;
    --leads) leads=1; pat="" ;;
  esac
done
if [ -z "$doc" ]; then
  echo "usage: section.sh <doc> [heading-regex] [--all] | section.sh <doc> --leads | section.sh --find <regex> [doc]" >&2
  exit 2
fi

f=""
for c in "$doc" "$root/$doc" "$trp/docs/$doc.md" "$trp/docs/spec/$doc.md" \
         "$trp/docs/$doc" "$trp/docs/spec/$doc" "$root/$doc.md" "$trp/$doc"; do
  if [ -f "$c" ]; then f="$c"; break; fi
done
if [ -z "$f" ] && [[ "$doc" == *.rs ]]; then
  f="$(find "$trp/crates" -path '*/src/*' -name "$(basename "$doc")" | sed -n 1p)"
fi
if [ -z "$f" ]; then
  echo "no such document: $doc" >&2
  exit 1
fi
if [ -f "$trp/docs/$doc.md" ] && [ -f "$trp/docs/spec/$doc.md" ]; then
  echo "(both docs/$doc.md and docs/spec/$doc.md exist: this is docs/$doc.md; \"spec/$doc\" names the other)"
fi

# A source file: its banners (or its top-level items), or one banner's range.
if [[ "$f" == *.rs ]]; then
  if ! grep -q '^// ==== ' "$f"; then
    echo "$f has no // ==== banners; its top-level items:"
    grep -nE '^(pub(\([a-z]+\))? )?(fn|struct|enum|impl|trait|mod|const|static|type) ' "$f" | cut -c1-100
    exit 0
  fi
  if [ -z "$pat" ]; then
    awk '
      { sub(/\r$/, "") }
      /^\/\/ ==== .* ====$/ { if (h != "") printf "%6d-%-6d %5d lines  %s\n", start, NR - 1, NR - start, h; h = $0; sub(/^\/\/ ==== /, "", h); sub(/ ====$/, "", h); start = NR }
      END { if (h != "") printf "%6d-%-6d %5d lines  %s\n", start, NR, NR - start + 1, h; printf "%s: %d lines. section.sh <file.rs> \"<banner words>\" prints one range (or Read it with offset and limit).\n", FILENAME, NR }
    ' "$f"
    exit 0
  fi
  awk -v pat="$pat" -v file="$f" '
    { sub(/\r$/, "") }
    /^\/\/ ==== .* ====$/ { if (printing) { printing = 0 } if (!done && tolower($0) ~ tolower(pat)) { printing = 1; done = 1; first = NR } }
    printing { print; last = NR }
    END { if (!done) { printf "no banner matches /%s/ in %s\n", pat, file > "/dev/stderr"; exit 1 } printf "== lines %d-%d of %s ==\n", first, last, file }
  ' "$f"
  exit 0
fi

if [ -z "$pat" ]; then
  # The headings, each with its first line, its last line and its size in bytes; with --leads, the
  # paragraphs under each that open in bold (the specs' own index of rules), cut at 90 characters.
  awk -v leads="$leads" '
    { sub(/\r$/, "") }
    /^```/ { fence = !fence; bytes += length($0) + 1; total += length($0) + 1; next }
    fence { bytes += length($0) + 1; total += length($0) + 1; next }
    /^#+ / { flush(); h = $0; start = NR; bytes = 0; body = "" }
    leads && /^(- |[0-9]+\. )?\*\*[^*]+\*\*/ && !/^#/ {
      l = $0; sub(/^(- |[0-9]+\. )?\*\*/, "", l); sub(/\*\*.*$/, "", l)
      if (length(l) > 90) l = substr(l, 1, 87) "..."
      body = body sprintf("          %6d    %s\n", NR, l)
    }
    { bytes += length($0) + 1; total += length($0) + 1 }
    function flush() {
      if (h == "") return
      printf "%6d-%-6d %7d B  %s\n", start, NR - 1, bytes, h
      if (body != "") printf "%s", body
    }
    END { if (h != "") { NR++; flush() }
          printf "%s: %d lines, %d bytes. section.sh <doc> \"<heading>\" prints one section.\n", FILENAME, NR - 1, total }
  ' "$f"
  exit 0
fi

# One section: from the first heading that matches to the next heading of its level or higher.
out="$(awk -v pat="$pat" -v file="$f" '
  { sub(/\r$/, "") }
  /^```/ { fence = !fence }
  !fence && /^#+ / {
    match($0, /^#+/); lvl = RLENGTH
    if (printing && lvl <= plvl) { printing = 0 }
    if (!printing && !done && tolower($0) ~ tolower(pat)) {
      printing = 1; done = 1; plvl = lvl; first = NR
    } else if (done && !printing && tolower($0) ~ tolower(pat)) {
      others = others sprintf("  line %d  %s\n", NR, $0)
    }
  }
  printing { print; last = NR }
  END {
    if (!done) { printf "no heading matches /%s/ in %s; section.sh %s lists them\n", pat, file, file > "/dev/stderr"; exit 1 }
    printf "== lines %d-%d of %s ==\n", first, last, file
    if (others != "") printf "also matching, not printed:\n%s", others
  }
' "$f")"
size=${#out}
if [ "$all" = 0 ] && [ "$size" -gt 40000 ]; then
  # Not "| head": with pipefail, head's early close would end the script before the note.
  sed -n '1,20p' <<< "$out"
  echo "== ... $size bytes (about $((size / 4)) tokens) in this section: its first 20 lines shown. --all prints it whole; better, --find a word in it and Read a few lines around the hit. =="
  tail -n 2 <<< "$out"
else
  printf '%s\n' "$out"
fi
