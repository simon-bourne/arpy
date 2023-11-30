use xtask_base::{
    ci::{Tasks, CI},
    generate_open_source_files,
    github::actions::{install, rust_toolchain, Platform},
    CommonCmds,
};

fn main() {
    let codegen = |check| generate_open_source_files(2021, check);
    CommonCmds::run(ci(), codegen)
}

fn ci() -> CI {
    let mut workflow = CI::new();
    let stable_rustc = || rust_toolchain("1.73").minimal().default().wasm();
    let wasm_pack = || install("wasm-pack", "0.12.1");

    for platform in Platform::latest() {
        workflow.add_job(
            Tasks::new("tests", platform, stable_rustc().clippy())
                .step(wasm_pack())
                .codegen()
                .tests(None),
        );
        workflow.add_job(
            Tasks::new("release-tests", platform, stable_rustc())
                .step(wasm_pack())
                .release_tests(None),
        );
    }

    workflow.standard_lints("nightly-2023-10-14", "0.1.43", &[])
}
