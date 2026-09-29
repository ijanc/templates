#!/bin/sh
# The core crates may only depend on what is listed here. A web framework,
# a database driver or the runtime showing up means a port was bypassed;
# such code belongs in an adapter.
set -eu

cd "$(dirname "$0")/.."

check() {
	crate=$1
	shift
	cargo tree -p "$crate" -e normal --depth 1 --prefix none \
		| awk 'NR > 1 { print $1 }' | sort -u | while read -r dep; do
		case " $* " in
			*" $dep "*) ;;
			*)
				echo "$crate depends on $dep, which is not allowed" >&2
				exit 1
				;;
		esac
	done
}

check {{project-name}}-domain async-trait chrono thiserror uuid
check {{project-name}}-application {{project-name}}-domain async-trait thiserror uuid
echo "layers ok"
