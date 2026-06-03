; ModuleID = 'test2'
source_filename = "test2"

@str = private unnamed_addr constant [6 x i8] c"Hello\00", align 1
@str.1 = private unnamed_addr constant [3 x i8] c", \00", align 1
@str.2 = private unnamed_addr constant [7 x i8] c"World!\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [4 x i8] c"foo\00", align 1
@str.4 = private unnamed_addr constant [4 x i8] c"bar\00", align 1
@print_fmt_str.5 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define ptr @main() {
entry:
  %concat = call ptr @hulk_concat(ptr @str, ptr @str.1)
  %concat1 = call ptr @hulk_concat(ptr %concat, ptr @str.2)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %concat1)
  %concat_full = call ptr @hulk_concat_full(ptr @str.3, ptr @str.4)
  %print2 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.5, ptr %concat_full)
  ret ptr %concat_full
}

declare ptr @hulk_concat(ptr, ptr)

declare i32 @printf(ptr, ...)

declare ptr @hulk_concat_full(ptr, ptr)
