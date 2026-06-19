# Informe del Compilador HULK

Kelen Alfaro Garcia                 C311, 
Adriana Boue Garcia                 C311, 
Carlos Alejandro Mazorra Matos      C311

## 1. Visión general de la arquitectura

El compilador está organizado como una cadena de procesamiento clásica en varias etapas:

1. Lectura del archivo de entrada.
2. Preprocesamiento léxico.
3. Tokenización con `logos`.
4. Postprocesamiento de tokens para inyecciones sintéticas y correcciones de forma.
5. Parsing con `lalrpop` para construir un AST explícito.
6. Análisis semántico para resolver tipos, declaraciones y referencias.
7. Desugaring y expansión de macros para convertir formas sintácticas complejas en AST canónico.
8. Generación de código intermedio y optimizaciones hacia LLVM.
9. Emisión de código LLVM y creación de binarios.


## 2. Arquitectura del AST

El AST (Abstract Syntax Tree) de HULK es la representación intermedia tipada del programa. Está diseñado para ser:

- Simple de construir desde el parser.
- Estable durante las fases de análisis (minimizar mutaciones directas).
- Suficientemente expresivo para guardar la semántica necesaria por las fases posteriores (tipos, referencias, trazabilidad).

### 2.1. Estructura principal

El nodo raíz es `Program`:

- `Program { items: Vec<Item> }`

`Item` recoge tanto declaraciones como expresiones globales:

- `Import(ImportDecl)`
- `Export(ExportDecl)`
- `Function(FunctionDecl)`
- `Type(TypeDecl)`
- `Protocol(ProtocolDecl)`
- `Macro(MacroDecl)`
- `GlobalExpr(Expr)`

Esta separación permite que pases (por ejemplo, análisis de declaraciones, resolución de símbolos o desugaring) trabajen uniformemente sobre la lista de `items` sin ramas especiales.

### 2.2. Declaraciones principales

Las declaraciones se representan con structs específicos que contienen la información necesaria para cada caso:

- `ImportDecl` / `ExportDecl`: ruta/imports, alias y metadata.
- `FunctionDecl`: identificador, lista de parámetros (nombre + `TypeRef` opcional), tipo de retorno opcional y cuerpo (`Expr` o `Block`).
- `TypeDecl`: nombre del tipo, parámetros genéricos, lista de miembros (campos y métodos) y herencia/implements.
- `ProtocolDecl`: interfaz que expone firmas; puede heredar otros protocolos.
- `MacroDecl`: nombre, parámetros especiales (`@`, `$`, `*`), y cuerpo que suele ser una expresión o bloque que el macro expandirá.

Estas estructuras modelan el espacio de nombres y sirven como entradas para la fase de resolución de símbolos y la inferencia de tipos.

### 2.3. Tipos y anotaciones

Los tipos se representan con `TypeRef`:

- `Number`, `String`, `Boolean` — tipos primitivos.
- `Vector(Box<TypeRef>)` — vectores/postfix `T[]` (soporta anidamiento: `T[][]`).
- `Function(Vec<TypeRef>, Box<TypeRef>)` — tipos de función `(T1, T2) -> R`.
- `Custom(String)` — referencia a tipos definidos por el usuario.


### 2.4. Expresiones y `KindExpr`

Las expresiones usan un wrapper uniforme:

```rust
pub struct Expr {
    pub id: NodeId,
    pub span: Span,
    pub kind: KindExpr,
}
```

Separar `kind` de `id`/`span` permite mantener metadatos (identificadores únicos y posición en fuente) sin mezclarlos con la semántica de la expresión.

La fábrica `mk_expr(kind, span)` centraliza la creación de nodos: asigna un `NodeId` fresco, fija el `span` y devuelve la `Expr` completa.

#### 2.4.1. `KindExpr` (variantes)

`KindExpr` es un `enum` exhaustivo:

- `Literal(LiteralExpr)`
- `Variable(VariableExpr)`
- `Binary(BinaryExpr)`
- `Unary(UnaryExpr)`
- `Call(CallExpr)`
- `BaseCall(BaseCallExpr)`
- `MacroCall(MacroCallExpr)`
- `Let(LetExpr)`
- `Block(BlockExpr)`
- `If(IfExpr)`
- `While(WhileExpr)`
- `For(ForExpr)`
- `Assign(AssignExpr)`
- `MemberAccess(MemberAccessExpr)`
- `Index(IndexExpr)`
- `Array(ArrayExpr)`
- `ArrayComprehension(ArrayComprehensionExpr)`
- `Lambda(LambdaExpr)`
- `New(NewExpr)`
- `Is(IsExpr)`
- `As(AsExpr)`
- `Match(MatchExpr)`

Agregar variantes exige actualizar pases dependientes (parser, semántica, codegen). Mantener `match` exhaustivos ayuda al compilador a forzar esos cambios por compilación fallida en lugar de errores silenciosos.

### 2.5. Expresiones específicas (detalles útiles)

- `BinaryExpr` contiene campos `left: Expr`, `op: BinaryOp`, `right: Expr`.
- `CallExpr` contiene `callee: Expr` y `args: Vec<Expr>`; si los argumentos usan marcas (`@`, `$`) la acción del parser puede producir `MacroCallExpr`.
- `ArrayExpr` contiene `elements: Vec<Expr>` para literales.
- `ArrayComprehensionExpr` conserva la forma sintáctica original (element, variable, iterable, condición opcional) para facilitar trazabilidad y generación diferida.
- `LambdaExpr` guarda parámetros y el cuerpo (expresión o bloque).

Las estructuras recursivas usan `Box` o `Vec` según convenga para evitar tipos de tamaño infinito en Rust.

### 2.6. Identificadores, spans y trazabilidad

- `NodeId` es un `u32` (wrapped) usado como clave estable en mapas de análisis (`HashMap<NodeId, ...`) para tipos, símbolos, inferencia y anotaciones.
- `Span { start, end }` guarda posiciones en bytes o en offsets de `Location` (según la convención del lexer) y se usa para diagnosticar errores.




## 3. Arquitectura del lexer

El módulo de lexing usa `logos`, una biblioteca de lexer orientada a patrones, que convierte el texto fuente en una secuencia de tokens. En HULK esta capa se mantiene deliberadamente simple: su objetivo es reconocer los símbolos léxicos válidos, descartar ruido e informar errores de caracteres no reconocidos. La semántica se resuelve después, en el parser y el preprocesador.

### 3.1. Definición de token

El enum `Token` en `src/lexer.rs` define los tokens reconocidos por el lexer. Cada variante puede corresponder a un token fijo, un token con payload o una regla de ignorado.

- Se ignoran espacios en blanco y saltos con `#[logos(skip "[ \t\r\n\f]+")]`.
- Se descartan comentarios de línea `//...` y bloques `/* ... */`.
- Literales y símbolos se reconocen y se transforman en tokens con los valores necesarios.

La enumeración incluye categorías clave:

- keywords: `import`, `export`, `let`, `if`, `else`, `while`, `for`, `function`, `type`, `new`, `inherits`, `is`, `as`, `protocol`, `interface`, `extends`, `def`, `define`, `match`, `case`, `default`, `self`, `true`, `false`.
- tipos: `Number`, `String`, `Boolean`.
- operadores aritméticos: `Plus`, `Minus`, `Star`, `Slash`, `Caret`, `Mod`.
- operadores de comparación: `EqualEqual`, `NotEqual`, `LessEqual`, `GreaterEqual`, `Less`, `Greater`.
- operadores lógicos y especiales: `And`, `Or`, `Not`, `Equal`, `ColonEqual`, `At`, `DoubleAt`, `DoublePipe`, `Dollar`.
- delimitadores y símbolos: `LParen`, `RParen`, `LBrace`, `RBrace`, `LBracket`, `RBracket`, `Comma`, `Dot`, `Colon`, `Semicolon`, `Arrow`, `ThinArrow`.
- literales: `Identifier(String)`, `Number(f64)`, `String(String)`.

En `logos`, el orden de las variantes importa cuando dos patrones pueden solaparse. Por ejemplo, `==` debe reconocerse antes que `=` y `>=` antes que `>`. HULK mantiene esa prioridad en la definición de `Token` y en el orden de los atributos para evitar coincidencias incorrectas.

### 3.2. Reglas especiales

El lexer aplica reglas concretas a los literales:

- Identificadores siguen `[a-zA-Z][a-zA-Z0-9_]*`.
- Números siguen `[0-9]+(\.[0-9]+)?`; es decir, admiten enteros y decimales simples.
- Strings usan delimitadores `"..."` e interpretan secuencias escapadas limitadas: `\n`, `\t`, `\r`, `\\` y `\"`.

Además:

- Los literales numéricos se parsean directamente a `f64` para que el parser reciba valores ya convertidos.
- Las cadenas se desescapan en el lexer y se guardan como `String` limpio.
- Los comentarios no generan tokens; se eliminan antes de que el parser procese la entrada.



## 4. Preprocesador y normalizaciones antes del parser

El módulo `src/preprocessor.rs` es clave para la robustez del front-end. Su responsabilidad es aplicar validaciones y transformaciones que simplifican el parser.

### 4.1. Validaciones específicas
`validate_array_initializer_syntax` es una comprobación temprana cuyo objetivo no es solamente rechazar código mal formado, sino asegurar que la sintaxis de inicializadores de array llega al parser en la forma esperada.

En HULK la forma original del inicializador de arrays usa `{ ... }` para distinguirse de llamadas y otros bloques. Antes de que el parser procese el código, el preprocesador puede convertir esa sintaxis original a una forma interna menos ambigua, como `(...)` o `[]`, cuando es seguro hacerlo.

Por eso el validador hace dos cosas:

- Rechaza directamente expresiones que ya usan `()` o `[]` en lugar de la forma original `{}` en contextos de inicializador, porque ese código no debería escribirse así y puede producir ambigüedad.
- Señala el error antes de que el lexer/parser intente procesarlo, para evitar diagnósticos confusos derivados de reglas sintácticas equivocadas.

Descripción ampliada:

- Caso 1: `new Type[5]( i -> i + 1 )` — esta forma parece un inicializador lambda, pero la sintaxis original válida es `new Type[5]{ i -> i + 1 }`. Por ello, el validador rechaza `new Type[5](...)` y obliga al usuario a escribir `{...}`. Si el código usa `{...}`, el preprocesador podrá normalizarlo a la forma interna que el parser acepta.

- Caso 2: `let a: Number[] = [10, 20, 30]` — el literal de array válido en el dialecto original se escribe con llaves `{10, 20, 30}`. Por eso el validador marca este uso como inválido y sugiere la forma correcta.


### 4.2. Normalización de sintaxis azucarada

El preprocesador implementa transformaciones sintácticas (azúcar) de forma explícita y local para que el parser trabaje sobre una sintaxis más regular y previsible. Estas transformaciones se realizan en una o dos pasadas sobre el texto fuente y se documentan para que sean reproducibles y fáciles de probar.

Transformaciones principales:

- `new Type[expr]{ ... }` → `new Type[expr]( ... )`: cuando detectamos el patrón de un inicializador lambda escrito con llaves, lo convertimos a una forma con paréntesis y una función lambda explícita. Esta normalización evita que el parser tenga que soportar una producción adicional para la forma con llaves y facilita la desambiguación entre llamadas y inicializadores.

- `{ a, b, c }` → `[ a, b, c ]` para literales de array: en los casos donde el contenido del bloque corresponde claramente a una lista de expresiones separadas por comas y no a un bloque de código (por ejemplo, no contiene declaraciones `let` o `if`), el preprocesador convierte llaves en corchetes para unificar la representación de arrays literales en el parser.

Detalles y cuidado con los límites:

- Las normalizaciones solo se aplican cuando el preprocesador puede decidir de forma local y segura que no está transformando un bloque de código válido. Por ejemplo, si un bloque contiene declaraciones o expresiones que no son compatibles con un literal de array, no se transforma.
- Las transformaciones quedan registradas (logs de preprocesador) para facilitar depuración y pruebas automáticas.

Beneficios:

- Reduce la complejidad de la gramática y el número de producciones necesarias en `parser.lalrpop`.
- Permite dar errores o sugerencias coherentes al usuario cuando el código usa formas no compatibles.


### 4.4. Decisión de diseño del preprocesador

La decisión de extraer las transformaciones sintácticas y las comprobaciones tempranas a un módulo independiente (`src/preprocessor.rs`) responde a motivos prácticos y de mantenibilidad:

- Simplicidad del lexer: permitimos que el lexer sea una máquina de estados orientada a reconocer patrones lexemáticos y no una capa que intente resolver ambigüedades sintácticas.
- Reducción de la gramática: al normalizar azúcar y casos especiales antes del parsing, `parser.lalrpop` puede permanecer conciso y se evitan producciones excepcionales que complican la tabla de parseo.
- Mejora en la calidad de los diagnósticos: el preprocesador puede detectar patrones mal formados y proporcionar mensajes de error más claros y específicos que si esos problemas se detectaran como errores sintácticos genéricos dentro del parser.

Motivación concreta (errores de ambigüedad):

En iteraciones previas del proyecto se observaron múltiples fallos donde pequeñas variaciones sintácticas inducían conflictos en lalrpop (shift/reduce o reduce/reduce). Para mitigar esto sin sacrificar expresividad, se decidió mover la responsabilidad de desambiguación a una capa previa, donde las transformaciones son explícitas y fáciles de testear.

Impacto en el flujo de compilación:

- El flujo queda más modular: lectura → preprocesador → lexer → postprocesador de tokens → parser.
- Facilita la reutilización y pruebas: el preprocesador se puede ejecutar en pruebas unitarias aisladas para verificar que normalizaciones y errores se gestionan correctamente.


## 5. Arquitectura del parser

`src/parser.lalrpop` define la gramática del lenguaje y mapea sus producciones directamente a nodos del AST. Esta capa transforma la secuencia de tokens en un árbol rico en estructura, manteniendo información de ubicación y de origen para diagnóstico posterior.

### 5.1. Integración con tokens y AST

El parser declara la interfaz de tokens para `lalrpop` con:

- `extern { type Location = usize; type Error = (); enum Token { ... } }`

Esto le dice a `lalrpop` cómo leer tokens del lexer y detectar errores de análisis. Cada producción del parser devuelve un `Expr`, `Item`, `TypeRef` u otro nodo del AST, y usa helpers como `mk_expr(...)` para asignar `NodeId`/`Span` automáticamente.

El parser no crea estructuras auxiliares complejas: cada expresión se construye inmediatamente como un nodo AST. Por ejemplo, una regla de suma puede compilarse en `mk_expr(KindExpr::Binary(BinaryExpr { left, operator, right }), span)`, enlazando subexpresiones y preservando la posición en el código fuente.

### 5.2. Programa y elementos principales

El nodo raíz `Program` puede representarse en distintas formas según el contenido del archivo:

- `TopLevelItems` seguido de `GlobalExprItems`.
- Solo `TopLevelItems`.
- Una sola `Expr` cuando el archivo contiene únicamente una expresión.

Esto permite escribir programas con declaraciones primero y luego código ejecutable global, o bien scripts cortos que son solo una expresión.

`TopLevelItem` agrupa las declaraciones válidas en la parte superior del archivo:

- `ImportDecl`
- `ExportDecl`
- `MacroDecl`
- `ProtocolDecl`
- `TypeDecl`
- `FunctionDecl`

Al mismo tiempo, el símbolo `Item` admite `Expr ";"` como un elemento más del programa. Esto es importante porque habilita archivos en los que se mezclan definiciones y ejecución inmediata, como:

```hulk
import foo
let x = 5;
print(x);
```

Con este diseño, el parser puede generar un AST coherente aun cuando el código combine declaraciones de tipo y expresiones de ejecución.

### 5.3. Declaraciones detalladas

La gramática soporta declaraciones complejas:

- `MacroDecl` con `def` o `define`, permitiendo parámetros especiales `*`, `@`, `$` y cuerpo en expresión o bloque.
- `TypeDecl` con parámetros genéricos, herencia `inherits` y miembros que pueden ser campos o métodos.
- `ProtocolDecl`/`Interface` con herencia de protocolos y firmas de método.
- `FunctionDecl` con cuerpo que puede ser una sola expresión o bloque de múltiples expresiones.

Este conjunto de opciones hace posible un lenguaje con macros de primera clase, tipos estructurados y extensibilidad por herencia/protocolos.

### 5.4. Expresiones y precedencia

El parser organiza la expresión en niveles de precedencia para que las operaciones se interpreten con el orden esperado:

1. `LogOrExpr` y `LogAndExpr` para `|` y `&`.
2. `EqExpr` para `==`, `!=`.
3. `CmpExpr` para `<`, `>`, `<=`, `>=`.
4. `TypeExpr` para `is` y `as`.
5. `StringExpr` para concatenación con `@` y `@@`.
6. `ArithExpr` para suma y resta.
7. `MulExpr` para multiplicación, división y módulo.
8. `PowExpr` para exponentes.
9. `UnaryExpr` para negación y not.
10. `CallExpr` para llamadas, acceso a miembros y indexación.
11. `PrimaryExpr` para literales, variables, arrays, paréntesis y comprensiones.

Esta jerarquía evita ambigüedades comunes en expresiones mixtas y mantiene el parser alineado con expectativas de los programadores.

Un ejemplo de la estructura interna sería una expresión como:

```hulk
1 + x * y - z
```

En esta expresión, el operador `*` tiene mayor precedencia que `+` y `-`, por lo que se evalúa primero. El parser interpreta la expresión así:

- `ArithExpr` principal con izquierda `1` y derecha `x * y - z`.
- Dentro de la parte derecha, `MulExpr` con `x` y `y` se evalúa antes que la suma/resta.
- Después se combina el resultado de `x * y` con `1` y, finalmente, se resta `z`.

### 5.5. Constructores especiales

El parser incluye reglas dedicadas para construcciones de control y formas sintácticas especiales:

- `IfExpr`, `WhileExpr`, `ForExpr`, `LetExpr`, `BlockExpr`.
- `LambdaExpr` con la forma `function(...) -> expr`.
- `NewExpr` con varias variantes:
  - `new BaseType (args)` para instancias normales.
  - `new BaseType [size] (params -> body)` para inicializadores lambda de arrays.
  - `new BaseType [size]` y `new BaseType []` para arrays sin inicializador.
- `MatchExpr` para coincidencia de patrones, inclusivo de literales y operadores.

Estas producciones permiten mapear la sintaxis de alto nivel directamente a nodos AST bien tipados.

### 5.6. Manejo de macros y bloques

El parser distingue llamadas normales de macros basándose en argumentos marcados:

- `MacroMarkedArgList` acepta argumentos prefijados con `@` (argumento simbólico) o `$` (placeholder).
- Si el callee es identificador y la lista de argumentos contiene marcas, se construye un `MacroCallExpr`.
- En caso contrario, se construye un `CallExpr` normal.

Esto hace posible que sintaxis como `repeat(@x)` o `print($msg)` se parseen como macros, mientras que `foo(x, y)` sigue siendo una llamada normal.

La separación en el parser es esencial porque las macros pueden tener un comportamiento de expansión distinto y deben conservar información específica de sus argumentos marcados.

### 5.7. Tipos y arreglos

La gramática de tipos admite:

- tipos básicos y personalizados: `Number`, `String`, `Boolean`, `Custom(name)`.
- vectores con sufijo `[]`, que pueden repetirse para representar `T[][]`.
- tipos de función con forma `(T1, T2) -> R`.

Un tipo como `Number[][]` se modela como `Vector(Vector(Number))`, lo que permite representar anidamiento de arrays de forma natural.

### 5.8. Literales, variables y arrays

`PrimaryExpr` cubre los casos primarios más simples:

- literales numéricas, de cadena y booleanas.
- `self` y variables identificadas.
- expresiones entre paréntesis.
- literales de array y comprensiones.

Las comprensiones de arrays admiten la sintaxis:

```hulk
[ element | variable in iterable ]
```

El parser crea un nodo `ArrayComprehensionExpr` que conserva el elemento, la variable, el iterable y cualquier condición. Esta forma de alto nivel se mantiene hasta que la fase de desugaring o codegen decide cómo bajar la comprensión a código ejecutable.

### 5.9. Diseño y extensibilidad

El parser está construido para ser modular y fácil de extender:

- Cambios en la gramática se reflejan directamente en las reglas de `parser.lalrpop`.
- Las acciones se mantienen pequeñas y producen nodos AST simples.
- Las nuevas construcciones se integran añadiendo variantes de AST y reglas de parseo, con un impacto mínimo en el resto del pipeline.

El resultado es una cadena de parseo que mantiene la sintaxis del lenguaje clara y el AST expresivo, al tiempo que deja la mayor parte de la semántica compleja para fases posteriores.


