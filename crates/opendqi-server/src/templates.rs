//! Minijinja templates for the OpenDQI desktop UI. Templates are
//! embedded at compile time so the binary is self-contained.

use minijinja::Environment;

const INDEX_TEMPLATE: &str = include_str!("../templates/index.html");
const RESULTS_TEMPLATE: &str = include_str!("../templates/results.html");
const BASE_TEMPLATE: &str = include_str!("../templates/base.html");

pub fn build_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_template("base.html", BASE_TEMPLATE)
        .expect("compile base template");
    env.add_template("index.html", INDEX_TEMPLATE)
        .expect("compile index template");
    env.add_template("results.html", RESULTS_TEMPLATE)
        .expect("compile results template");
    env
}
