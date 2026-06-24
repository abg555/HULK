; ModuleID = 'functions'
source_filename = "functions"

@str = private unnamed_addr constant [8 x i8] c"Hello, \00", align 1
@str.1 = private unnamed_addr constant [2 x i8] c"!\00", align 1
@str.2 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.7 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.8 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.9 = private unnamed_addr constant [5 x i8] c"HULK\00", align 1
@print_fmt_str.10 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @double(double %x) {
entry:
  %x1 = alloca double, align 8
  store double %x, ptr %x1, align 8
  %load_x = load double, ptr %x1, align 8
  %multmp = fmul double %load_x, 2.000000e+00
  ret double %multmp
}

define ptr @greet(ptr %name) {
entry:
  %name1 = alloca ptr, align 8
  store ptr %name, ptr %name1, align 8
  %load_name = load ptr, ptr %name1, align 8
  %concat = call ptr @hulk_concat(ptr @str, ptr %load_name)
  %concat2 = call ptr @hulk_concat(ptr %concat, ptr @str.1)
  ret ptr %concat2
}

define double @fib(double %n) {
entry:
  %n1 = alloca double, align 8
  store double %n, ptr %n1, align 8
  %load_n = load double, ptr %n1, align 8
  %cmptmp = fcmp ole double %load_n, 1.000000e+00
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %load_n2 = load double, ptr %n1, align 8
  br label %if_merge

if_else:                                          ; preds = %entry
  %load_n3 = load double, ptr %n1, align 8
  %subtmp = fsub double %load_n3, 1.000000e+00
  %call_fib = call double @fib(double %subtmp)
  %load_n4 = load double, ptr %n1, align 8
  %subtmp5 = fsub double %load_n4, 2.000000e+00
  %call_fib6 = call double @fib(double %subtmp5)
  %addtmp = fadd double %call_fib, %call_fib6
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp = phi double [ %load_n2, %if_then ], [ %addtmp, %if_else ]
  ret double %iftmp
}

declare ptr @hulk_concat(ptr, ptr)

define i32 @main() {
entry:
  %call_double = call double @double(double 7.000000e+00)
  %cmptmp = fcmp oeq double %call_double, 1.400000e+01
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str.2)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str.2, %if_then ], [ @str.3, %if_else ]
  %call_fib = call double @fib(double 1.000000e+01)
  %cmptmp2 = fcmp oeq double %call_fib, 5.500000e+01
  br i1 %cmptmp2, label %if_then3, label %if_else4

if_then3:                                         ; preds = %if_merge
  %print6 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge5

if_else4:                                         ; preds = %if_merge
  %print7 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.8, ptr @str.7)
  br label %if_merge5

if_merge5:                                        ; preds = %if_else4, %if_then3
  %iftmp_str8 = phi ptr [ @str.5, %if_then3 ], [ @str.7, %if_else4 ]
  %call_greet = call ptr @greet(ptr @str.9)
  %print9 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.10, ptr %call_greet)
  ret i32 0
}

declare i32 @printf(ptr, ...)
