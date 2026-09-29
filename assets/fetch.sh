#!/usr/bin/env bash
# Re-download every pack listed in packs.toml, verify its sha256, and unpack only the files
# the asset library uses into packs/<id>/ (plus packs/<id>/LICENSE.txt).
#
#   assets/fetch.sh              fetch every pack
#   assets/fetch.sh <id>...      fetch only these packs
#
# Needs: bash, curl, unzip, sha256sum (Git Bash on Windows has all of them).
# Downloads are cached in ${DROID_ASSET_CACHE:-${TMPDIR:-/tmp}/droid-asset-cache}.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
cache="${DROID_ASSET_CACHE:-${TMPDIR:-/tmp}/droid-asset-cache}"
mkdir -p "$cache"

# packs.toml -> lines "P|id|url|sha256|licence_file|licence|page|title|author" and "F|id|file".
# (`|` rather than tabs: `read` would merge empty tab-separated fields.)
parse() {
  awk '
    function val(line) { sub(/^[^=]*= *"/, "", line); sub(/" *,? *$/, "", line); return line }
    function flush() { if (id != "") printf "P|%s|%s|%s|%s|%s|%s|%s|%s\n", id, url, sha, lic, licence, page, title, author }
    /^\[\[pack\]\]/ { flush(); id=""; url=""; sha=""; lic=""; licence=""; page=""; title=""; author=""; infiles=0; next }
    /^id *=/ { id=val($0); next }
    /^url *=/ { url=val($0); next }
    /^sha256 *=/ { sha=val($0); next }
    /^licence_file *=/ { lic=val($0); next }
    /^licence *=/ { licence=val($0); next }
    /^page *=/ { page=val($0); next }
    /^title *=/ { title=val($0); next }
    /^author *=/ { author=val($0); next }
    /^files *= *\[/ { infiles=1; next }
    infiles && /^\]/ { infiles=0; next }
    infiles && /"/ { f=$0; sub(/^ *"/, "", f); sub(/" *,? *$/, "", f); printf "F|%s|%s\n", id, f; next }
    END { flush() }
  ' "$here/packs.toml"
}

want_ids=("$@")
is_wanted() {
  [ "${#want_ids[@]}" -eq 0 ] && return 0
  local w
  for w in "${want_ids[@]}"; do [ "$w" = "$1" ] && return 0; done
  return 1
}

table="$(parse)"
fail=0
while IFS='|' read -r kind id url sha lic licence page title author; do
  [ "$kind" = "P" ] || continue
  is_wanted "$id" || continue
  name="$(basename "${url%%\?*}")"
  dl="$cache/$id-$name"
  if [ ! -f "$dl" ] || [ "$(sha256sum "$dl" | cut -d' ' -f1)" != "$sha" ]; then
    echo "download $id  <- $url"
    curl -fsSL -A "Mozilla/5.0 (droid asset fetch)" -o "$dl.part" "$url"
    mv "$dl.part" "$dl"
  fi
  got="$(sha256sum "$dl" | cut -d' ' -f1)"
  if [ "$got" != "$sha" ]; then
    echo "error: $id: sha256 is $got, packs.toml says $sha" >&2
    fail=1
    continue
  fi
  dest="$here/packs/$id"
  mkdir -p "$dest"
  mapfile -t files < <(printf '%s\n' "$table" | awk -F'|' -v id="$id" '$1=="F" && $2==id { print $3 }')
  case "$name" in
    *.zip)
      # -j would flatten; keep the paths the library refers to. Brackets are literal (-W not needed).
      unzip -qo "$dl" "${files[@]}" -d "$dest"
      if [ -n "$lic" ]; then
        unzip -p "$dl" "$lic" > "$dest/LICENSE.txt"
      fi
      ;;
    *)
      mkdir -p "$(dirname "$dest/${files[0]}")"
      cp "$dl" "$dest/${files[0]}"
      ;;
  esac
  if [ -z "$lic" ]; then
    printf '%s\nby %s\n\nLicence: %s (Creative Commons Zero, public domain dedication)\nhttps://creativecommons.org/publicdomain/zero/1.0/\n\nSource: %s\nDownloaded from: %s\n' \
      "$title" "$author" "$licence" "$page" "$url" > "$dest/LICENSE.txt"
  fi
  echo "ok       $id  (${#files[@]} files)"
done <<< "$table"

exit "$fail"
