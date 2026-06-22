# Ejemplos de Prueba para HULK

Estos ejemplos están limitados al subconjunto que el codegen soporta hoy: números, booleanos, funciones tipadas, llamadas, `print`, `let`, bloques, `if` y recursión simple.

## Nivel 1: Funciones y llamadas

### 1.1 - Suma tipada
```js
function add(a: Number, b: Number): Number => a + b;

add(2, 3)
```
**Esperado:** 5

---

### 1.2 - Llamadas anidadas
```js
function square(x: Number): Number => x * x;
function quad(x: Number): Number => square(square(x));

quad(2)
```
**Esperado:** 16

---

### 1.3 - Mezcla de builtin math
```js
function math_combo(x: Number): Number => sqrt(x) + sin(PI / 2) + cos(0) + log(2, 8) + exp(0);

math_combo(16)
```
**Esperado:** 10

---

## Nivel 2: Control de flujo y tipos booleanos

### 2.1 - if/else con comparación
```js
function abs_value(x: Number): Number => if (x > 0) x else 0 - x;

abs_value(-7)
```
**Esperado:** 7

---

### 2.2 - Selección por condición
```js
function choose(x: Number): Number => if (x >= 10) x - 10 else x + 10;

choose(7)
```
**Esperado:** 17

---

### 2.3 - Fibonacci recursivo
```js
function fib(n: Number): Number => if (n <= 1) n else fib(n - 1) + fib(n - 2);

fib(7)
```
**Esperado:** 13

---

## Nivel 3: `let` y bloques

### 3.1 - `let` anidado
```js
function compute(x: Number): Number =>
  let a: Number = x * 2 in
  let b: Number = a + 5 in
  b * b;

compute(3)
```
**Esperado:** 121

---

### 3.2 - Bloque con `print`
```js
function show(x: Number): Number => {
  print(x);
  print(x * 2);
  print(x ^ 2);
  x ^ 2
};

show(5)
```
**Esperado:** imprime 5, 10, 25 y retorna 25

---

### 3.3 - Bloque con variables locales
```js
function nested(x: Number): Number => {
  let a: Number = x + 1 in {
    print(a);
    let b: Number = a * 2 in {
      print(b);
      b + a
    }
  }
};

nested(5)
```
**Esperado:** imprime 6, 12 y retorna 18

---

## Nivel 4: Recursión y estrés razonable

### 4.1 - Factorial
```js
function factorial(n: Number): Number => if (n <= 1) 1 else n * factorial(n - 1);

factorial(5)
```
**Esperado:** 120

---

### 4.2 - Potencia recursiva
```js
function power(base: Number, exp: Number): Number => if (exp == 0) 1 else base * power(base, exp - 1);

power(2, 10)
```
**Esperado:** 1024

---

### 4.3 - Suma acumulada por composición
```js
function add1(x: Number): Number => x + 1;
function add2(x: Number): Number => add1(add1(x));
function add4(x: Number): Number => add2(add2(x));

add4(10)
```
**Esperado:** 14

---

## Sugerencias de prueba

1. Copia un ejemplo en el campo `input` de `src/bin/ex.rs`.
2. Ejecuta `cargo run --bin ex`.
3. Verifica que el IR generado corresponda al ejemplo elegido.
4. Si quieres validar la salida real en consola, usa un programa que llame a `print(...)`.

## Qué se evitó a propósito

- Cadenas de texto
- Arrays, protocolos, clases y `new`
- Tipos inferidos ambiguos o `Unknown`
- Features que todavía no están bajadas por el codegen actual
