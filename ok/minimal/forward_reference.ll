; ModuleID = 'forward_reference'
source_filename = "forward_reference"

@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @f() {
entry:
  %call_g = call double @g()
  ret double %call_g
}

define double @g() {
entry:
  ret double 4.200000e+01
}

define i32 @main() {
entry:
  %call_f = call double @f()
  %cmptmp = fcmp oeq double %call_f, 4.200000e+01
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  %call_g = call double @g()
  %cmptmp2 = fcmp oeq double %call_g, 4.200000e+01
  br i1 %cmptmp2, label %if_then3, label %if_else4

if_then3:                                         ; preds = %if_merge
  %print6 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge5

if_else4:                                         ; preds = %if_merge
  %print7 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge5

if_merge5:                                        ; preds = %if_else4, %if_then3
  %iftmp_str8 = phi ptr [ @str.3, %if_then3 ], [ @str.5, %if_else4 ]
  ret i32 0
}

declare i32 @printf(ptr, ...)
