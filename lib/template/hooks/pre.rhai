// Derive crate metadata from the built-in variables.
let name = variable::get("project-name");
variable::set("crate_name", to_snake_case(name));

let d = system::date();
variable::set("year", d.year.to_string());

file::delete("hooks");
