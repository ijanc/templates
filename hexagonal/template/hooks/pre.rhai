// Derive crate metadata from the built-in variables.
let name = variable::get("project-name");
let crate_name = to_snake_case(name);

variable::set("crate_name", crate_name);
variable::set("env_prefix", to_shouty_snake_case(crate_name));

let d = system::date();
variable::set("year", d.year.to_string());

// The in-memory store always stays: it is the test double of the core
// and of the HTTP adapter. Only the database stores get the sqlx one.
let store = variable::get("store");
let sqlx = store != "memory";
variable::set("sqlx", sqlx);
if !sqlx {
    file::delete("adapters/store-sqlx");
}

// Only asked for the database stores.
if !variable::is_set("cli") {
    variable::set("cli", false);
}
if !variable::get("cli") {
    file::delete("apps/cli");
}

file::delete("hooks");
