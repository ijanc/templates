[private]
default:
    @just --list

# Regenerate every *-generated project
generate-all:
    just generate-daemon

# Regenerate daemon-generated from ./daemon
generate-daemon $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf daemon-generated
    cargo generate --path ./daemon \
        --name daemon-generated --vcs none \
        --define project-description="An example generated using the daemon template" \
        --define gh-username=ijanc \
        --define domain=example.org

# Check every *-generated project
check:
    cargo check --workspace --tests
    for d in *-generated; do mandoc -Tlint -Werror $d/*.8 $d/*.5; done
