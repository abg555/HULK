; ModuleID = 'tests'
source_filename = "tests"

@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @main() {
entry:
  %count = alloca double, align 8
  store double 0.000000e+00, ptr %count, align 8
  %for_result = alloca double, align 8
  store double 0.000000e+00, ptr %for_result, align 8
  %for_i_0 = alloca double, align 8
  store double 0.000000e+00, ptr %for_i_0, align 8
  br label %for_cond

for_cond:                                         ; preds = %for_body, %entry
  %for_i_load = load double, ptr %for_i_0, align 8
  %for_cmp = fcmp olt double %for_i_load, 1.000000e+01
  br i1 %for_cmp, label %for_body, label %for_after

for_body:                                         ; preds = %for_cond
  %i = alloca double, align 8
  store double %for_i_load, ptr %i, align 8
  %load_count = load double, ptr %count, align 8
  %addtmp = fadd double %load_count, 1.000000e+00
  store double %addtmp, ptr %count, align 8
  %reload_count = load double, ptr %count, align 8
  store double %reload_count, ptr %for_result, align 8
  %for_next = fadd double %for_i_load, 1.000000e+00
  store double %for_next, ptr %for_i_0, align 8
  br label %for_cond

for_after:                                        ; preds = %for_cond
  %for_result1 = load double, ptr %for_result, align 8
  %load_count2 = load double, ptr %count, align 8
  %cmptmp = fcmp oeq double %load_count2, 1.000000e+01
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %for_after
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %for_after
  %print3 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  ret double 0.000000e+00
}

declare i32 @printf(ptr, ...)
