use anyhow::Result;
use inkwell::context::Context;
use seppolang::{parse_seppo, CodeGen};
use std::fs;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use std::process;

fn compile_and_run(input: &str) -> Result<i64> {
    // Initialize LLVM targets
    inkwell::targets::Target::initialize_all(&inkwell::targets::InitializationConfig {
        asm_parser: true,
        asm_printer: true,
        base: true,
        disassembler: true,
        info: true,
        machine_code: true,
    });
    
    // Initialize native target
    inkwell::targets::Target::initialize_native(&inkwell::targets::InitializationConfig::default())
        .map_err(|e| anyhow::anyhow!("Failed to initialize native target: {}", e))?;

    // Parse
    let expr = parse_seppo(input)?;

    // Create a unique temporary directory for this test run
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = process::id();
    let temp_dir = env::temp_dir()
        .join(format!("seppolang_test_{}_{}", pid, timestamp));
    fs::create_dir_all(&temp_dir)?;
    
    let obj_file = temp_dir.join("test.o");
    let exe_file = temp_dir.join("test");

    println!("Using temp directory: {:?}", temp_dir);
    println!("Object file path: {:?}", obj_file);
    println!("Executable file path: {:?}", exe_file);

    // Generate code
    let context = Context::create();
    let mut codegen = CodeGen::new(&context, "test");
    codegen.compile(&expr)?;

    // Write object file
    codegen.write_object_file(&obj_file)?;

    // Debug: Print object file size
    println!("Object file size: {} bytes", fs::metadata(&obj_file)?.len());

    // Verify object file was created
    if !obj_file.exists() {
        return Err(anyhow::anyhow!("Object file was not created"));
    }

    // Link with more verbose error handling
    let mut link_command = std::process::Command::new("cc");
    link_command
        .arg("-o")
        .arg(&exe_file)
        .arg(&obj_file);
    
    // Add any C object files
    for c_obj in codegen.c_object_files() {
        link_command.arg(c_obj);
    }
    
    let output = link_command.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("Linking failed:");
        eprintln!("stderr: {}", stderr);
        eprintln!("stdout: {}", stdout);
        fs::remove_file(&obj_file)?;
        return Err(anyhow::anyhow!("Linking failed: {}", stderr));
    }

    // Verify executable was created
    if !exe_file.exists() {
        return Err(anyhow::anyhow!("Executable file was not created"));
    }

    // Run with more verbose error handling
    let output = std::process::Command::new(&exe_file)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute binary: {}", e))?;

    // Get the exit code, ensuring it's properly captured
    let exit_code = output.status.code()
        .ok_or_else(|| anyhow::anyhow!("Process terminated by signal"))?;

    // Print debug information
    println!("Program stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("Program stderr: {}", String::from_utf8_lossy(&output.stderr));
    println!("Exit code: {}", exit_code);

    // Clean up
    fs::remove_file(&obj_file)?;
    fs::remove_file(&exe_file)?;
    fs::remove_dir(&temp_dir)?;

    Ok(exit_code as i64)
}

#[test]
fn test_main_return() -> Result<()> {
    let input = r#"
        fn main() {
            return 42
        }
    "#;
    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_main_implicit_return() -> Result<()> {
    let input = r#"
        fn main() {
            x = 42
        }
    "#;
    assert_eq!(compile_and_run(input)?, 0);
    Ok(())
}

#[test]
fn test_variable_assignment() -> Result<()> {
    let input = r#"
        fn main() {
            x = 42
            return x
        }
    "#;
    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_print_statement() -> Result<()> {
    let input = r#"
        fn main() {
            x = 42
            seppo x
            return 0
        }
    "#;
    assert_eq!(compile_and_run(input)?, 0);
    Ok(())
}

#[test]
fn test_arithmetic() -> Result<()> {
    let input = r#"
        fn main() {
            x = 40
            y = 2
            return x + y
        }
    "#;
    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_multiple_statements() -> Result<()> {
    let input = r#"
        fn main() {
            x = 40
            y = 2
            z = x + y
            seppo z
            return z
        }
    "#;
    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
#[should_panic(expected = "No main function found")]
fn test_missing_main() {
    let input = r#"
        fn not_main() {
            return 42
        }
    "#;
    compile_and_run(input).unwrap();
}

#[test]
#[should_panic(expected = "Undefined variable")]
fn test_undefined_variable() {
    let input = r#"
        fn main() {
            return x
        }
    "#;
    compile_and_run(input).unwrap();
}

#[test]
fn test_inline_c_function() -> Result<()> {
    let input = r#"
        ceppo {
            long my_rand() {
                return 42;
            }
        }

        fn main() {
            x = my_rand()
            seppo x
            return x
        }
    "#;

    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}
