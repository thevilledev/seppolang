mod codegen;
mod parser;
mod types;

use anyhow::{anyhow, Result};
use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target};
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::path::Path;
use std::process::Command;

fn compile_file(input: &Path, output: &Path) -> Result<()> {
    let content = std::fs::read_to_string(input)?;
    println!("Compiling {} to {}", input.display(), output.display());

    // Parse the input
    let expr = parser::parse_seppo(&content)?;

    // Initialize LLVM
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| anyhow!("Failed to initialize LLVM: {}", e))?;

    // Generate code
    let context = Context::create();
    let mut codegen = codegen::CodeGen::new(&context, input.file_name().unwrap().to_str().unwrap());
    codegen.compile(&expr)?;

    // Verify module
    if codegen.get_module().verify().is_err() {
        return Err(anyhow!("Module verification failed"));
    }

    // Write LLVM IR (optional, for debugging)
    codegen
        .get_module()
        .print_to_file(output.with_extension("ll"))
        .map_err(|e| anyhow!("Failed to write LLVM IR: {}", e.to_string()))?;

    // Generate object file
    let obj_file = output.with_extension("o");
    codegen.write_object_file(&obj_file)?;

    // Link the object file
    let output_exe = output.with_extension(EXE_SUFFIX);
    link_object_file(&obj_file, &output_exe)?;

    // Clean up intermediate files
    std::fs::remove_file(&obj_file)
        .map_err(|e| anyhow!("Failed to clean up object file: {}", e))?;

    println!("Successfully compiled to {}", output_exe.display());
    Ok(())
}

fn link_object_file(obj_file: &Path, output: &Path) -> Result<()> {
    let status = if cfg!(target_os = "windows") {
        Command::new("link")
            .args(&[
                "/NOLOGO",
                "/DEFAULTLIB:libcmt",
                "/SUBSYSTEM:CONSOLE",
                &format!("/OUT:{}", output.display()),
                &obj_file.display().to_string(),
            ])
            .status()
    } else {
        let mut cmd = Command::new("cc");

        cmd.arg("-v");

        if cfg!(target_os = "macos") {
            // Add all library paths first
            /*cmd.args(&[
                "-L/opt/homebrew/lib",
                "-L/opt/homebrew/opt/zstd/lib",
                "-L/opt/homebrew/opt/llvm@18/lib",
                "-L/opt/homebrew/Cellar/llvm@18/18.1.8/lib",
            ]);*/

            // Add LLVM library path from Homebrew
            if let Ok(output) = Command::new("brew").args(["--prefix", "llvm"]).output() {
                if let Ok(llvm_path) = String::from_utf8(output.stdout) {
                    let llvm_path = llvm_path.trim();
                    cmd.arg(format!("-L{}/lib", llvm_path));
                }
            }

            // Add macOS SDK path - fix the isysroot format
            if let Ok(output) = Command::new("xcrun").args(["--show-sdk-path"]).output() {
                if let Ok(sdk_path) = String::from_utf8(output.stdout) {
                    cmd.arg(format!("-isysroot{}", sdk_path.trim())); // Removed the = sign
                }
            }

            // Add input and output files
            cmd.args(&["-o", output.to_str().unwrap(), obj_file.to_str().unwrap()]);

            // Add libraries in specific order
            /*cmd.args(&[
                "-lzstd",     // zstd first
                "-lLLVM",     // then LLVM
                "-lSystem",   // explicitly add System library
            ]);*/
        } else {
            // Linux version
            cmd.args(&[
                "-o",
                output.to_str().unwrap(),
                obj_file.to_str().unwrap(),
                //"-lz", "-lstdc++", "-lc", "-lm", "-lzstd", "-lLLVM"
            ]);
        }

        println!("Running linker command: {:?}", cmd);
        cmd.status()
    }
    .map_err(|e| anyhow!("Failed to execute linker: {}", e))?;

    if !status.success() {
        return Err(anyhow!("Linking failed with status: {}", status));
    }

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    match args.as_slice() {
        [_, input] => {
            let input_path = Path::new(input);
            let output = input_path.with_extension("");
            compile_file(input_path, &output)?;
        }
        [_, input, output] => {
            compile_file(Path::new(input), Path::new(output))?;
        }
        _ => {
            println!("Usage: seppoc input.seppo [output]");
        }
    }

    Ok(())
}
