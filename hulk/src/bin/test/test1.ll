; ModuleID = 'test1'
source_filename = "test1"

@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define ptr @main() {
entry:
  %i = alloca double, align 8
  store double 0.000000e+00, ptr %i, align 8
  %result = alloca double, align 8
  store double 0.000000e+00, ptr %result, align 8
  %while_result = alloca double, align 8
  store double 0.000000e+00, ptr %while_result, align 8
  br label %while_cond

while_cond:                                       ; preds = %while_body, %entry
  %load_i = load double, ptr %i, align 8
  %cmptmp = fcmp olt double %load_i, 5.000000e+00
  br i1 %cmptmp, label %while_body, label %while_after

while_body:                                       ; preds = %while_cond
  %load_result = load double, ptr %result, align 8
  %load_i1 = load double, ptr %i, align 8
  %addtmp = fadd double %load_result, %load_i1
  store double %addtmp, ptr %result, align 8
  %reload_result = load double, ptr %result, align 8
  %load_i2 = load double, ptr %i, align 8
  %addtmp3 = fadd double %load_i2, 1.000000e+00
  store double %addtmp3, ptr %i, align 8
  %reload_i = load double, ptr %i, align 8
  store double %reload_i, ptr %while_result, align 8
  br label %while_cond

while_after:                                      ; preds = %while_cond
  %while_result4 = load double, ptr %while_result, align 8
  %load_result5 = load double, ptr %result, align 8
  %cmptmp6 = fcmp oeq double %load_result5, 1.000000e+01
  br i1 %cmptmp6, label %if_then, label %if_else

if_then:                                          ; preds = %while_after
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %while_after
  %print7 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  %load_i8 = load double, ptr %i, align 8
  %cmptmp9 = fcmp oeq double %load_i8, 5.000000e+00
  br i1 %cmptmp9, label %if_then10, label %if_else11

if_then10:                                        ; preds = %if_merge
  %print13 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge12

if_else11:                                        ; preds = %if_merge
  %print14 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge12

if_merge12:                                       ; preds = %if_else11, %if_then10
  %iftmp_str15 = phi ptr [ @str.3, %if_then10 ], [ @str.5, %if_else11 ]
  ret ptr %iftmp_str15
}

declare i32 @printf(ptr, ...)
