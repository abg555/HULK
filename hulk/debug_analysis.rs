// Archivo de prueba simple: compilar solo las transformaciones del lexer
// Copiar el contenido de src/main.rs pero omitir parser/semantic/codegen

use std::fs;

// Copiar las funciones necesarias del lexer aquí para debuggear

fn main() {
    let input = r#"let evens = 0 in {
    for (i in range(0, 10)) {
        evens := evens + if (i % 2 == 0) 1 else 0;
    };
    if (evens == 5) print("ok") else print("fail");
};"#;

    println!("=== INPUT ===");
    println!("{}", input);
    println!();
    
    println!("=== ANÁLISIS ===");
    
    // Buscar el problema: "+" seguido de "if"
    let problem_str = "+ if";
    if let Some(pos) = input.find(problem_str) {
        println!("Encontré '+ if' en posición {}", pos);
        println!("Línea: {}", input[..pos].lines().count());
    }
    
    // Verificar la transformación esperada
    println!();
    println!("El lexer debería convertir:");
    println!("  evens + if (...) expr else expr");
    println!("en:");
    println!("  evens + ( if (...) expr else expr )");
}
