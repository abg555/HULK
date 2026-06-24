; ModuleID = 'block_value'
source_filename = "block_value"

@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define i32 @main() {
entry:
  %a = alloca double, align 8
  store double 3.000000e+00, ptr %a, align 8
  %b = alloca double, align 8
  store double 4.000000e+00, ptr %b, align 8
  %load_a = load double, ptr %a, align 8
  %load_b = load double, ptr %b, align 8
  %addtmp = fadd double %load_a, %load_b
  %result = alloca double, align 8
  store double %addtmp, ptr %result, align 8
  %load_result = load double, ptr %result, align 8
  %cmptmp = fcmp oeq double %load_result, 7.000000e+00
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  %load_result2 = load double, ptr %result, align 8
  %multmp = fmul double %load_result2, 2.000000e+00
  %inner = alloca double, align 8
  store double %multmp, ptr %inner, align 8
  %load_inner = load double, ptr %inner, align 8
  %cmptmp3 = fcmp oeq double %load_inner, 1.400000e+01
  br i1 %cmptmp3, label %if_then4, label %if_else5

if_then4:                                         ; preds = %if_merge
  %print7 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge6

if_else5:                                         ; preds = %if_merge
  %print8 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge6

if_merge6:                                        ; preds = %if_else5, %if_then4
  %iftmp_str9 = phi ptr [ @str.3, %if_then4 ], [ @str.5, %if_else5 ]
  ret i32 0
}

declare i32 @printf(ptr, ...)
