[private]
default:
    @just --list

# Regenerate every *-generated project
generate-all:
    just generate-daemon
    just generate-cli
    just generate-cli-clap
    just generate-api
    just generate-api-sqlite
    just generate-api-postgres

# Regenerate daemon-generated from ./daemon
generate-daemon $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf daemon-generated
    cargo generate --path ./daemon \
        --name daemon-generated --vcs none \
        --define project-description="An example generated using the daemon template" \
        --define gh-username=ijanc \
        --define domain=example.org

# Regenerate cli-generated from ./cli (getopt, config, dotenv)
generate-cli $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf cli-generated
    cargo generate --path ./cli \
        --name cli-generated --vcs none \
        --define project-description="An example generated using the cli template" \
        --define gh-username=ijanc \
        --define domain=example.org \
        --define args=getopt \
        --define config=true \
        --define dotenv=true

# Regenerate cli-clap-generated from ./cli (clap, no config, no dotenv)
generate-cli-clap $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf cli-clap-generated
    cargo generate --path ./cli \
        --name cli-clap-generated --vcs none \
        --define project-description="An example generated using the cli template" \
        --define gh-username=ijanc \
        --define domain=example.org \
        --define args=clap \
        --define config=false \
        --define dotenv=false

# Regenerate api-generated from ./api (in-memory store)
generate-api $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf api-generated
    cargo generate --path ./api \
        --name api-generated --vcs none \
        --define project-description="An example generated using the api template" \
        --define gh-username=ijanc \
        --define domain=example.org \
        --define store=memory

# Regenerate api-sqlite-generated from ./api (sqlx, SQLite, moka cache)
generate-api-sqlite $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf api-sqlite-generated
    cargo generate --path ./api \
        --name api-sqlite-generated --vcs none \
        --define project-description="An example generated using the api template" \
        --define gh-username=ijanc \
        --define domain=example.org \
        --define store=sqlite \
        --define cache=true

# Regenerate api-postgres-generated from ./api (sqlx, PostgreSQL)
generate-api-postgres $CARGO_NAME="your name" $CARGO_EMAIL="author@example.com":
    rm -rf api-postgres-generated
    cargo generate --path ./api \
        --name api-postgres-generated --vcs none \
        --define project-description="An example generated using the api template" \
        --define gh-username=ijanc \
        --define domain=example.org \
        --define store=postgres \
        --define cache=false

# Check every *-generated project
check:
    cargo check --workspace --tests
    for d in *-generated; do \
        if ls $d/*.[1-8] >/dev/null 2>&1; then \
            mandoc -Tlint -Werror $d/*.[1-8] || exit 1; \
        fi; \
    done
