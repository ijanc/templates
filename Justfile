default:
    @just --list

generate-all:
    just web
    just lib
    just cli


cli $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rv cli-generated
    cargo generate --path ./cli \
        --name cli-generated \
        --define project-description="An example generated using the cli template" \
        --define use-gitserver=false

web $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rv web-generated
    cargo generate --path ./web \
        --name web-generated \
        --define project-description="An example generated using the web template" \
        --define use-gitserver=false

lib $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rv lib-generated
    cargo generate --path ./lib \
        --name lib-generated \
        --define project-description="An example generated using the lib template" \
        --define use-gitserver=false

