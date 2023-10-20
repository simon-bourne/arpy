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
    for platform in Platform::latest() {
        workflow.add_job(
            Tasks::new(
                "tests",
                platform,
                rust_toolchain("1.73").minimal().default().clippy().wasm(),
            )
            .step(install("wasm-pack", "0.12.1"))
            .tests(),
        );
    }

    workflow.standard_lints("nightly-2023-10-14", "0.1.43")
}
