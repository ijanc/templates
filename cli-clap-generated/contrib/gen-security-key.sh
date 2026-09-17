#!/bin/sh
# Generate a PGP key for security reports and splice it into SECURITY.md.
# Idempotent: reuses an existing key for the same user id.
set -eu

uid="cli-clap-generated Security <security@example.org>"
file="$(dirname "$0")/../SECURITY.md"
marker='<!-- pgp-key -->'

if ! grep -q "$marker" "$file"; then
	echo "$0: marker $marker not found in $file" >&2
	exit 1
fi

if ! gpg --batch --list-keys "=$uid" >/dev/null 2>&1; then
	gpg --batch --quick-gen-key "$uid" ed25519 sign 0
	fingerprint=$(gpg --batch --with-colons --list-keys "=$uid" | awk -F: '$1 == "fpr" { print $10; exit }')
	gpg --batch --quick-add-key "$fingerprint" cv25519 encr 0
fi

fingerprint=$(gpg --batch --with-colons --list-keys "=$uid" | awk -F: '$1 == "fpr" { print $10; exit }')
pretty=$(echo "$fingerprint" | sed 's/..../& /g; s/ $//')
key=$(gpg --batch --armor --export "$fingerprint")

tmp=$(mktemp)
awk -v marker="$marker" -v fingerprint="$pretty" -v key="$key" '
	$0 == marker {
		print "Fingerprint: `" fingerprint "`"
		print ""
		print "```"
		print key
		print "```"
		next
	}
	{ print }
' "$file" >"$tmp"
mv "$tmp" "$file"
echo "$fingerprint"
