// Derive crate metadata from the built-in variables.
let name = variable::get("project-name");
let crate_name = to_snake_case(name);

variable::set("crate_name", crate_name);
variable::set("env_prefix", to_shouty_snake_case(crate_name));

// "Name <email>" -> separate fields.
let authors = variable::get("authors");
let lt = authors.index_of("<");
if lt > 0 {
    let author_name = authors.sub_string(0, lt);
    author_name.trim();
    let author_email = authors.sub_string(lt + 1);
    author_email.remove(">");
    author_email.trim();
    variable::set("author_name", author_name);
    variable::set("author_email", author_email);
} else {
    variable::set("author_name", authors);
    variable::set("author_email", "");
}

let d = system::date();
variable::set("year", d.year.to_string());

// Keep only the selected store as src/store.rs.
let store = variable::get("store");
let sqlx = store != "memory";
variable::set("sqlx", sqlx);
if sqlx {
    file::rename("src/store_sqlx.rs", "src/store.rs");
    file::delete("src/store_memory.rs");
} else {
    file::rename("src/store_memory.rs", "src/store.rs");
    file::delete("src/store_sqlx.rs");
    file::delete("migrations");
}

// Without accounts there is nothing to log in to.
let auth = variable::get("auth");
if auth == "none" {
    file::delete("src/auth.rs");
    file::delete("src/routes/auth.rs");
    file::delete("templates/auth");
    file::delete("templates/invites");
    file::delete("tests/auth.rs");
    if sqlx {
        file::delete("migrations/0002_users.up.sql");
        file::delete("migrations/0002_users.down.sql");
        file::delete("migrations/0003_invites.up.sql");
        file::delete("migrations/0003_invites.down.sql");
    }
}

file::delete("hooks");
