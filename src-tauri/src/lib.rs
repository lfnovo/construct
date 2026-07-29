#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
mod okf;
mod okf_lint;
mod okf_policy;

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
mod diagnostics;
#[cfg(feature = "desktop")]
mod index;
#[cfg(feature = "desktop")]
mod knowledge;
#[cfg(feature = "desktop")]
mod mcp;
#[cfg(feature = "desktop")]
mod terminal;

pub(crate) const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".gradle",
    ".idea",
    "Pods",
    "DerivedData",
    "bin",
    "obj",
    ".terraform",
    ".dart_tool",
    ".pub-cache",
    "coverage",
    ".coverage",
];

#[cfg(feature = "desktop")]
pub fn run() {
    desktop::run()
}

#[cfg(feature = "desktop")]
pub fn run_service_command(arguments: &[String]) -> Result<(), String> {
    knowledge::run_service_command(arguments)
}

#[cfg(feature = "desktop")]
pub fn run_mcp_command(arguments: &[String]) -> Result<(), String> {
    mcp::run_mcp_command(arguments)
}

pub fn run_okf_command(arguments: &[String]) -> Result<i32, String> {
    okf_lint::run_command(arguments)
}
