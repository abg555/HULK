; ModuleID = 'simple_test'
source_filename = "simple_test"

@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @main() {
entry:
  %format_num = call ptr @hulk_format_number(double 1.000000e+00)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %format_num)
  ret double 1.000000e+00
}

declare i32 @printf(ptr, ...)

declare ptr @hulk_format_number(double)
