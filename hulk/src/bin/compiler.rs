use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use inkwell::context::Context;
use std::process::Command;
use hulk::code_gen::CodeGenerator;
use hulk::{parse_program, SemanticAnalyzer};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: {} <archivo.hulk> [opciones]", args[0]);
        eprintln!();
        eprintln!("Opciones:");
        eprintln!("  -o <salida>    Archivo de salida (por defecto: <entrada>.ll)");
        eprintln!("  --ir-only      Solo genera IR, no compila a ejecutable");
        eprintln!("  --verbose      Muestra información de depuración");
        std::process::exit(1);
    }

    let input_file = &args[1];
    let input_path = Path::new(input_file);

    // Validar que el archivo existe
    if !input_path.exists() {
        eprintln!("Error: El archivo '{}' no existe", input_file);
        std::process::exit(1);
    }

    // Validar que es un archivo .hulk
    if input_path.extension().map_or(true, |ext| ext != "hulk") {
        eprintln!("Advertencia: El archivo no tiene extensión .hulk");
    }

    // Parsear opciones
    let mut output_file = None;
    let mut verbose = false;
    let mut ir_only = false;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                if i + 1 < args.len() {
                    output_file = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: -o requiere un archivo de salida");
                    std::process::exit(1);
                }
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            "--ir-only" => {
                ir_only = true;
                i += 1;
            }
            _ => {
                eprintln!("Opción desconocida: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    // Determinar archivo de salida
    let output_path = if let Some(ref out) = output_file {
        PathBuf::from(out)
    } else {
        let mut path = input_path.to_path_buf();
        path.set_extension("ll");
        path
    };

    if verbose {
        println!("Compilando: {}", input_file);
        println!("Salida IR: {}", output_path.display());
    }

    // Leer archivo de entrada
    let source = match fs::read_to_string(input_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error al leer archivo: {}", e);
            std::process::exit(1);
        }
    };

    if verbose {
        println!("Código fuente ({} bytes):", source.len());
    }

    // Fase 1: Parsing
    let program = match parse_program(&source) {
        Ok(program) => {
            if verbose {
                println!("✓ Parsing exitoso");
            }
            program
        }
        Err(diagnostics) => {
            eprintln!("Error en parsing:");
            for diagnostic in diagnostics {
                eprintln!("  {}", diagnostic.message);
            }
            std::process::exit(1);
        }
    };

    // Fase 2: Análisis semántico
    let analysis = match SemanticAnalyzer::new().analyze(&program) {
        Ok(analysis) => {
            if verbose {
                println!("✓ Análisis semántico exitoso");
                println!("Tipos inferidos: {}", analysis.inferred_types.len());
            }
            analysis
        }
        Err(diagnostics) => {
            eprintln!("Error en análisis semántico:");
            for diagnostic in diagnostics {
                eprintln!("  {}", diagnostic.message);
            }
            std::process::exit(1);
        }
    };

    // Fase 3: Generación de código LLVM IR
    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, input_path.file_stem().unwrap_or_default().to_string_lossy().as_ref());

    if let Err(message) = codegen.codegen_program(&program, &analysis) {
        eprintln!("Error en generación de código: {}", message);
        std::process::exit(1);
    }

    if verbose {
        println!("✓ Generación de código LLVM IR exitosa");
    }

    // Guardar IR a archivo
    let ir_string = codegen.module().print_to_string().to_string();
    
    match fs::write(&output_path, &ir_string) {
        Ok(_) => {
            if verbose {
                println!("✓ IR guardado en: {}", output_path.display());
                println!("Tamaño del IR: {} bytes", ir_string.len());
            } else {
                println!("IR guardado en: {}", output_path.display());
            }
        }
        Err(e) => {
            eprintln!("Error al escribir archivo de salida: {}", e);
            std::process::exit(1);
        }
    }

    // Si no es --ir-only, también intentar compilar a objeto/ejecutable
    if !ir_only {
        if verbose {
            println!("Compilando IR a ejecutable...");
        }

        // Determinar nombres para objeto y ejecutable
        // Si el usuario pasó -o con extensión distinta a .ll, asumimos que es el ejecutable
        let exe_path = if output_path.extension().map_or(false, |ext| ext == "ll") {
            // usar mismo nombre sin extensión
            let mut p = output_path.clone();
            p.set_extension("");
            p
        } else {
            output_path.clone()
        };

        let object_path = exe_path.with_extension("o");

        // Compilar runtime.rs a una staticlib para que incluya std y sus dependencias
        let runtime_src = Path::new("runtime.rs");
        let runtime_lib = Path::new("libruntime.a");
        
        if runtime_src.exists() {
            let rustc_status = Command::new("rustc")
            .arg("--crate-type=staticlib")
                .arg(runtime_src)
                .arg("-o")
            .arg(runtime_lib)
                .output();

            match rustc_status {
                Ok(output) => {
                    if !output.status.success() {
                        eprintln!("Advertencia: No se pudo compilar runtime.rs");
                        if verbose && !output.stderr.is_empty() {
                            eprintln!("rustc stderr:\n{}", String::from_utf8_lossy(&output.stderr));
                        }
                    } else if verbose {
                        println!("✓ libruntime.a compilado");
                    }
                }
                Err(_) => {
                    if verbose {
                        eprintln!("Advertencia: rustc no encontrado, intentando sin runtime.rs");
                    }
                }
            }
        }

        // Ejecutar `llc -filetype=obj -relocation-model=pic <ir.ll> -o <out.o>`
        let llc_output = Command::new("llc")
            .arg("-filetype=obj")
            .arg("-relocation-model=pic")
            .arg(&output_path)
            .arg("-o")
            .arg(&object_path)
            .output();

        match llc_output {
            Ok(output) => {
                if !output.status.success() {
                    eprintln!("Error: `llc` falló con código {:?}", output.status.code());
                    if !output.stderr.is_empty() {
                        eprintln!("llc stderr:\n{}", String::from_utf8_lossy(&output.stderr));
                    }
                    std::process::exit(1);
                } else if verbose {
                    println!("✓ llc generó objeto: {}", object_path.display());
                }
            }
            Err(e) => {
                eprintln!("Error al ejecutar `llc`: {}. Asegúrate de tener LLVM instalado y `llc` en PATH.", e);
                std::process::exit(1);
            }
        }

        // Enlazar con `clang` para producir ejecutable
        let mut clang_cmd = Command::new("clang");
        clang_cmd.arg(&object_path);
        
        // Agregar libruntime.a si fue compilada exitosamente
        if runtime_src.exists() && runtime_lib.exists() {
            clang_cmd.arg(runtime_lib);
        }
        
        clang_cmd
            .arg("-lm")
            .arg("-o")
            .arg(&exe_path);
        
        let clang_status = clang_cmd.output();

        match clang_status {
            Ok(output) => {
                if !output.status.success() {
                    eprintln!("Error: `clang` falló con código {:?}", output.status.code());
                    if !output.stderr.is_empty() {
                        eprintln!("clang stderr:\n{}", String::from_utf8_lossy(&output.stderr));
                    }
                    std::process::exit(1);
                } else {
                    if verbose {
                        println!("✓ Ejecutable enlazado: {}", exe_path.display());
                        println!("Objeto: {}", object_path.display());
                    } else {
                        println!("Ejecutable: {}", exe_path.display());
                    }
                }
            }
            Err(e) => {
                eprintln!("Error al ejecutar `clang`: {}. Asegúrate de tener clang/gcc en PATH.", e);
                std::process::exit(1);
            }
        }
    }

    if verbose {
        println!("✓ Compilación completada exitosamente");
    }
}
