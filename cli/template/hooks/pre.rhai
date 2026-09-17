// Derive crate metadata from the built-in variables.
let name = variable::get("project-name");
let crate_name = to_snake_case(name);

variable::set("crate_name", crate_name);
variable::set("env_prefix", to_shouty_snake_case(crate_name));

// "Name <email>" -> separate fields for mdoc .An/.Mt.
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

let months = ["January", "February", "March", "April", "May", "June", "July",
    "August", "September", "October", "November", "December"];
let d = system::date();
variable::set("year", d.year.to_string());
variable::set("mdocdate", `${months[d.month - 1]} ${d.day} ${d.year}`);

// Keep only the selected argument parser as src/args.rs.
let args = variable::get("args");
file::rename("src/args_" + args + ".rs", "src/args.rs");
if args == "clap" {
    file::delete("src/args_getopt.rs");
} else {
    file::delete("src/args_clap.rs");
}

if !variable::get("config") {
    file::delete("src/config.rs");
    file::delete("contrib/{{project-name}}.conf");
    file::delete("{{project-name}}.conf.5");
}
if !variable::get("dotenv") {
    file::delete(".env.example");
}

file::delete("hooks");
