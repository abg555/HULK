; ModuleID = 'hello'
source_filename = "hello"

@str = private unnamed_addr constant [14 x i8] c"Hello, World!\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define i32 @main() {
entry:
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  ret i32 0
}

declare i32 @printf(ptr, ...)
