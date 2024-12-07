use anyhow::Result;
use inkwell::context::Context;
use seppolang::{parse_seppo, CodeGen};
use std::env;
use std::fs;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

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
    let temp_dir = env::temp_dir().join(format!("seppolang_test_{}_{}", pid, timestamp));
    fs::create_dir_all(&temp_dir)?;

    let obj_file = temp_dir.join("test.o");
    let exe_file = temp_dir.join("test");

    println!("Using temp directory: {:?}", temp_dir);
    println!("Object file path: {:?}", obj_file);
    println!("Executable file path: {:?}", exe_file);

    // Use a closure to ensure cleanup happens even on error
    let result = (|| {
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
            .arg("-v") // Add verbose output
            .arg("-o")
            .arg(&exe_file)
            .arg(&obj_file);

        // Add any C object files
        for c_obj in codegen.c_object_files() {
            println!("Adding C object file: {:?}", c_obj);
            link_command.arg(c_obj);
        }

        println!("Link command: {:?}", link_command);
        let output = link_command.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!("Linking failed:");
            eprintln!("stderr: {}", stderr);
            eprintln!("stdout: {}", stdout);
            eprintln!("Link command was: {:?}", link_command);
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
        let exit_code = output
            .status
            .code()
            .ok_or_else(|| anyhow::anyhow!("Process terminated by signal"))?;

        // Print debug information
        println!(
            "Program stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "Program stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        println!("Exit code: {}", exit_code);

        Ok(exit_code as i64)
    })();

    // Always clean up
    let _ = fs::remove_file(&obj_file);
    let _ = fs::remove_file(&exe_file);
    let _ = fs::remove_dir(&temp_dir);

    result
}

#[test]
fn test_seppo_return() -> Result<()> {
    let input = r#"
        fn seppo() {
            return 42
        }
    "#;
    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_seppo_implicit_return() -> Result<()> {
    let input = r#"
        fn seppo() {
            x = 42
        }
    "#;
    assert_eq!(compile_and_run(input)?, 0);
    Ok(())
}

#[test]
fn test_variable_assignment() -> Result<()> {
    let input = r#"
        fn seppo() {
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
        fn seppo() {
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
        fn seppo() {
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
        fn seppo() {
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
#[should_panic(expected = "No seppo function found")]
fn test_missing_seppo() {
    let input = r#"
        fn not_seppo() {
            return 42
        }
    "#;
    compile_and_run(input).unwrap();
}

#[test]
#[should_panic(expected = "Undefined variable")]
fn test_undefined_variable() {
    let input = r#"
        fn seppo() {
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

        fn seppo() {
            x = my_rand()
            seppo x
            return x
        }
    "#;

    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_ceppo_with_whitespace() -> Result<()> {
    let input = r#"
        ceppo    {
            int meaning_of_life()     {
                return     42;
            }
        }

        fn seppo() {
            return meaning_of_life()
        }
    "#;

    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_ceppo_complex_function() -> Result<()> {
    let input = r#"
        ceppo {
            int factorial(int n) {
                if (n <= 1) return 1;
                return n * factorial(n - 1);
            }
        }

        fn seppo() {
            result = factorial(5)
            seppo result
            return result
        }
    "#;

    assert_eq!(compile_and_run(input)?, 120);
    Ok(())
}

#[test]
fn test_ceppo_different_types() -> Result<()> {
    let input = r#"
        ceppo {
            int get_int() {
                return 42;
            }

            double calc(float x, int y) {
                return x + y;
            }

            void* alloc(size_t size) {
                return malloc(size);
            }
        }

        fn seppo() {
            x = get_int()
            return x
        }
    "#;

    assert_eq!(compile_and_run(input)?, 42);
    Ok(())
}

#[test]
fn test_ceppo_chibihash() -> Result<()> {
    let input = r#"
        ceppo {
            uint64_t chibihash64__load64le(const uint8_t *p) {
                return (uint64_t)p[0] <<  0 | (uint64_t)p[1] <<  8 |
                    (uint64_t)p[2] << 16 | (uint64_t)p[3] << 24 |
                    (uint64_t)p[4] << 32 | (uint64_t)p[5] << 40 |
                    (uint64_t)p[6] << 48 | (uint64_t)p[7] << 56;
            }

            uint64_t chibihash64(const void *keyIn, ptrdiff_t len, uint64_t seed) {
                const uint8_t *k = (const uint8_t *)keyIn;
                ptrdiff_t l = len;

                const uint64_t P1 = UINT64_C(0x2B7E151628AED2A5);
                const uint64_t P2 = UINT64_C(0x9E3793492EEDC3F7);
                const uint64_t P3 = UINT64_C(0x3243F6A8885A308D);

                uint64_t h[4] = { P1, P2, P3, seed };

                for (; l >= 32; l -= 32) {
                    for (int i = 0; i < 4; ++i, k += 8) {
                        uint64_t lane = chibihash64__load64le(k);
                        h[i] ^= lane;
                        h[i] *= P1;
                        h[(i+1)&3] ^= ((lane << 40) | (lane >> 24));
                    }
                }

                h[0] += ((uint64_t)len << 32) | ((uint64_t)len >> 32);
                if (l & 1) {
                    h[0] ^= k[0];
                    --l, ++k;
                }
                h[0] *= P2; h[0] ^= h[0] >> 31;

                for (int i = 1; l >= 8; l -= 8, k += 8, ++i) {
                    h[i] ^= chibihash64__load64le(k);
                    h[i] *= P2; h[i] ^= h[i] >> 31;
                }

                for (int i = 0; l > 0; l -= 2, k += 2, ++i) {
                    h[i] ^= (k[0] | ((uint64_t)k[1] << 8));
                    h[i] *= P3; h[i] ^= h[i] >> 31;
                }

                uint64_t x = seed;
                x ^= h[0] * ((h[2] >> 32)|1);
                x ^= h[1] * ((h[3] >> 32)|1);
                x ^= h[2] * ((h[0] >> 32)|1);
                x ^= h[3] * ((h[1] >> 32)|1);

                // moremur: https://mostlymangling.blogspot.com/2019/12/stronger-better-morer-moremur-better.html
                x ^= x >> 27; x *= UINT64_C(0x3C79AC492BA7B653);
                x ^= x >> 33; x *= UINT64_C(0x1C69B3F74AC4AE35);
                x ^= x >> 27;

                return x;
            }
        }

        fn seppo() {
            result = chibihash64("hello", 5, 42)
            seppo result
        }
    "#;

    assert_eq!(compile_and_run(input)?, 0);
    Ok(())
}

#[test]
fn test_conditional_format() -> Result<()> {
    let input = r#"
        fn seppo() {
            x = 42
            seppo x > 40 {
                x = 1
            }
            perkele {
                x = 0
            }
            return x
        }
    "#;
    assert_eq!(compile_and_run(input)?, 1);
    Ok(())
}

#[test]
fn test_conditional_less_than() -> Result<()> {
    let input = r#"
        fn seppo() {
            x = 30
            seppo x < 40 {
                x = 1
            }
            perkele {
                x = 0
            }
            return x
        }
    "#;
    assert_eq!(compile_and_run(input)?, 1);
    Ok(())
}

#[test]
fn test_conditional_equals() -> Result<()> {
    let input = r#"
        fn seppo() {
            x = 42
            seppo x == 42 {
                x = 1
            }
            perkele {
                x = 0
            }
            return x
        }
    "#;
    assert_eq!(compile_and_run(input)?, 1);
    Ok(())
}
