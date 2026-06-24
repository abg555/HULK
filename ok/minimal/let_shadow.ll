; ModuleID = 'let_shadow'
source_filename = "let_shadow"

@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.7 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.8 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.9 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.10 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define i32 @main() {
entry:
  %x = alloca double, align 8
  store double 1.000000e+01, ptr %x, align 8
  %load_x = load double, ptr %x, align 8
  %cmptmp = fcmp oeq double %load_x, 1.000000e+01
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  %x2 = alloca double, align 8
  store double 2.000000e+01, ptr %x2, align 8
  %load_x3 = load double, ptr %x2, align 8
  %cmptmp4 = fcmp oeq double %load_x3, 2.000000e+01
  br i1 %cmptmp4, label %if_then5, label %if_else6

if_then5:                                         ; preds = %if_merge
  %print8 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge7

if_else6:                                         ; preds = %if_merge
  %print9 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge7

if_merge7:                                        ; preds = %if_else6, %if_then5
  %iftmp_str10 = phi ptr [ @str.3, %if_then5 ], [ @str.5, %if_else6 ]
  %load_x11 = load double, ptr %x, align 8
  %cmptmp12 = fcmp oeq double %load_x11, 1.000000e+01
  br i1 %cmptmp12, label %if_then13, label %if_else14

if_then13:                                        ; preds = %if_merge7
  %print16 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.8, ptr @str.7)
  br label %if_merge15

if_else14:                                        ; preds = %if_merge7
  %print17 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.10, ptr @str.9)
  br label %if_merge15

if_merge15:                                       ; preds = %if_else14, %if_then13
  %iftmp_str18 = phi ptr [ @str.7, %if_then13 ], [ @str.9, %if_else14 ]
  ret i32 0
}

declare i32 @printf(ptr, ...)
