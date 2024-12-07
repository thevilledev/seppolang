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
    link_object_file(&obj_file, &output_exe, &codegen)?;

    // Clean up intermediate files
    std::fs::remove_file(&obj_file)
        .map_err(|e| anyhow!("Failed to clean up object file: {}", e))?;

    println!("Successfully compiled to {}", output_exe.display());
    Ok(())
}

fn link_object_file(obj_file: &Path, output: &Path, codegen: &codegen::CodeGen) -> Result<()> {
    // Create a basic link command
    let mut link_command = Command::new("cc");
    link_command
        .arg("-v")  // Add verbose output for debugging
        .arg("-o")
        .arg(output)
        .arg(obj_file);

    // Add any C object files from ceppo blocks
    // We need to pass the CodeGen instance here to access c_object_files
    for c_obj in codegen.c_object_files() {
        println!("Adding C object file: {:?}", c_obj);
        link_command.arg(c_obj);
    }

    println!("Running linker command: {:?}", link_command);
    let output = link_command.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("Linking failed:");
        eprintln!("stderr: {}", stderr);
        eprintln!("stdout: {}", stdout);
        eprintln!("Link command was: {:?}", link_command);
        return Err(anyhow!("Linking failed: {}", stderr));
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
