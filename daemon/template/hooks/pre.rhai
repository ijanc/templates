// Derive program names and metadata from the built-in variables.
let name = variable::get("project-name");
let crate_name = to_snake_case(name);
let daemon_name = name + "d";

variable::set("crate_name", crate_name);
variable::set("daemon_name", daemon_name);
variable::set("ctl_name", name + "ctl");
variable::set("env_prefix", to_shouty_snake_case(crate_name));
variable::set("plist_id", "org." + name + "." + daemon_name);

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

file::delete("hooks");
