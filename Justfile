default:
    @just --list

generate-all:
    just web

web $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rv web-generated
    cargo generate --path ./component \
        --name web-generated \
        --define project-description="An example generated using the component template" \
        --define use-gitserver=false

