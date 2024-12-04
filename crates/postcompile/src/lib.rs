use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::Path,
    process::Command,
};

use deps::Errored;

mod deps;
mod features;

pub enum Error {
    Build(String),
    Run(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Success,
    Failure,
}

#[derive(Debug)]
pub struct CompileOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl std::fmt::Display for CompileOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit status: {:?}\n", self.status)?;
        if !self.stderr.is_empty() {
            write!(f, "--- stderr \n{}\n", self.stderr)?;
        }
        if !self.stdout.is_empty() {
            write!(f, "--- stdout \n{}\n", self.stdout)?;
        }
        Ok(())
    }
}

pub fn compile_custom(tokens: &[u8], config: &Config) -> Result<CompileOutput, Errored> {
    let mut program = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    program.env("RUSTC_BOOTSTRAP", "1");
    program.arg("-Zunpretty=expanded");

    let rust_flags = std::env::var_os("RUSTFLAGS");

    if let Some(rust_flags) = &rust_flags {
        program.args(
            rust_flags
                .as_encoded_bytes()
                .split(|&b| b == b' ')
                .map(|flag| OsString::from(OsStr::from_bytes(flag))),
        );
    }

    program.arg("--out-dir");
    program.arg(config.tmp_dir.as_ref());
    program.arg("--crate-name");
    program.arg(config.function_name.split("::").last().unwrap_or("unnamed"));

    let tmp_file = Path::new(config.tmp_dir.as_ref()).join(format!("{}.rs", config.function_name));

    std::fs::write(&tmp_file, tokens).unwrap();

    program.arg(&tmp_file);
    program.envs(std::env::vars());

    deps::build_dependencies(config, &mut program)?;

    program.stderr(std::process::Stdio::piped());
    program.stdout(std::process::Stdio::piped());

    let child = program.spawn().unwrap();

    let output = child.wait_with_output().unwrap();

    Ok(CompileOutput {
        status: if output.status.success() {
            ExitStatus::Success
        } else {
            ExitStatus::Failure
        },
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    })
}

#[derive(Clone, Debug)]
pub struct Config {
    pub manifest: Cow<'static, Path>,
    pub target_dir: Cow<'static, Path>,
    pub tmp_dir: Cow<'static, Path>,
    pub function_name: Cow<'static, str>,
}

#[macro_export]
#[doc(hidden)]
macro_rules! _function_name {
    () => {{
        fn f() {}
        fn type_name_of_val<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let mut name = type_name_of_val(f).strip_suffix("::f").unwrap_or("");
        while let Some(rest) = name.strip_suffix("::{{closure}}") {
            name = rest;
        }
        name
    }};
}

#[doc(hidden)]
pub fn build_dir() -> &'static Path {
    Path::new(env!("OUT_DIR"))
}

#[doc(hidden)]
pub fn target_dir() -> &'static Path {
    build_dir()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

#[macro_export]
#[doc(hidden)]
macro_rules! _config {
    () => {{
        $crate::Config {
            manifest: ::std::borrow::Cow::Borrowed(::std::path::Path::new(env!("CARGO_MANIFEST_PATH"))),
            tmp_dir: ::std::borrow::Cow::Borrowed($crate::build_dir()),
            target_dir: ::std::borrow::Cow::Borrowed($crate::target_dir()),
            function_name: ::std::borrow::Cow::Borrowed($crate::_function_name!()),
        }
    }};
}

#[macro_export]
macro_rules! compile {
    ($($tokens:tt)*) => {
        $crate::compile_str!(stringify!($($tokens)*))
    };
}

#[macro_export]
macro_rules! compile_str {
    ($expr:expr) => {
        $crate::try_compile_str!($expr).expect("failed to compile")
    };
}

#[macro_export]
macro_rules! try_compile {
    ($($tokens:tt)*) => {
        $crate::try_compile_str!(stringify!($($tokens)*))
    };
}

#[macro_export]
macro_rules! try_compile_str {
    ($expr:expr) => {
        $crate::compile_custom($expr.as_bytes(), &$crate::_config!())
    };
}
