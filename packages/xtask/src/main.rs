use xtask_base::{ci::CI, generate_open_source_files, CommonCmds};

fn main() {
    // TODO: Use `CI::standard_workflow()` once release mode tests are working.
    CommonCmds::run(ci(), |check| generate_open_source_files(2021, check))
}

fn ci() -> CI {
    CI::new()
        .standard_tests("1.73")
        .standard_lints("nightly-2023-10-14", "0.1.43")
}
