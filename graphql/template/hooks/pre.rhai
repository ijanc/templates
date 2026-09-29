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

// Only asked for the database stores.
if !variable::is_set("cache") {
    variable::set("cache", false);
}
if !variable::get("cache") {
    file::delete("src/cache.rs");
}

file::delete("hooks");
