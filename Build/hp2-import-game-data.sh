#!/bin/bash
# Import a retail Harry Potter 2 installation into the runtime data root.
#
# Shipped inside HarryPotter2.app/Contents/Resources so the importer is
# reachable from a plain double-click in the DMG, from a Homebrew install, and
# from a build tree. Reads data files only: it never runs the original
# installer, the retail executable, or any bundled binary.
#
# The importer itself is the unmodified Build/prepare_retail_data.py, resolved
# at runtime in preference order:
#   1. HP2_IMPORT_SCRIPT     explicit override
#   2. this app bundle       Resources/prepare_retail_data.py
#   3. a source checkout     <repo>/Build/prepare_retail_data.py
# Exits non-zero when the import fails so a double-click leaves a visible
# failure in Terminal.

set -u

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
DEFAULT_OUTPUT="$HOME/Library/Application Support/Harry Potter 2/Data/Retail"

resolve_importer() {
    if [ -n "${HP2_IMPORT_SCRIPT:-}" ] && [ -f "${HP2_IMPORT_SCRIPT}" ]; then
        printf '%s\n' "${HP2_IMPORT_SCRIPT}"
        return 0
    fi
    local candidate
    for candidate in \
        "${SCRIPT_DIR}/prepare_retail_data.py" \
        "${SCRIPT_DIR}/../../../Build/prepare_retail_data.py"
    do
        if [ -f "${candidate}" ]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    done
    return 1
}

IMPORTER="$(resolve_importer)" || {
    echo "error: could not find prepare_retail_data.py." >&2
    echo "       Set HP2_IMPORT_SCRIPT to its full path and run this again." >&2
    exit 2
}

# The importer is Python. A stock Mac ships without python3 unless the Command
# Line Tools or Xcode are installed, and a double-clicked script must say so
# plainly rather than dying with "command not found".
if ! command -v python3 > /dev/null 2>&1; then
    cat >&2 << 'MSG'
error: python3 was not found on this Mac.

The game data importer needs Python 3, which macOS does not install by default.
Install it once with either:

  xcode-select --install      (Apple's Command Line Tools, ~1 GB)

or

  brew install python         (Homebrew, if you have it)

Then run this again.
MSG
    exit 2
fi

if [ "$#" -gt 0 ]; then
    exec python3 "${IMPORTER}" "$@"
fi

echo "Harry Potter 2 - game data import"
echo
echo "This copies the data files from a retail copy of the game that you own."
echo "It never runs the original installer or the old game executable."
echo

while true; do
    printf 'Your retail installation folder (or .zip/.tar/.7z archive): '
    if ! IFS= read -r source_path; then
        echo
        echo "error: no path entered." >&2
        exit 2
    fi
    source_path="${source_path/#\~/$HOME}"
    if [ -z "${source_path}" ]; then
        continue
    fi
    if [ ! -e "${source_path}" ]; then
        echo "error: no such file or directory: ${source_path}" >&2
        echo "       Check the path and try again." >&2
        continue
    fi
    break
done

echo
printf 'Import destination [%s]: ' "${DEFAULT_OUTPUT}"
if ! IFS= read -r output_path; then
    output_path=""
fi
output_path="${output_path:-${DEFAULT_OUTPUT}}"
output_path="${output_path/#\~/$HOME}"

echo
echo "Importing from: ${source_path}"
echo "            to: ${output_path}"
echo

if [ -d "${source_path}" ]; then
    set -- --retail-root "${source_path}"
else
    set -- --archive "${source_path}"
fi

python3 "${IMPORTER}" \
    "$@" \
    --output "${output_path}" \
    --profile retail-only \
    --link-mode copy

status=$?
echo
if [ "${status}" -eq 0 ]; then
    echo "Import complete."
    echo
    echo "Open HarryPotter2 and press New Game, or Continue for an existing save."
else
    echo "Import failed (exit ${status}). Fix the problem above and run this again;"
    echo "re-importing is safe and will not touch your saves."
fi
exit "${status}"
