; ModuleID = 'conditionals'
source_filename = "conditionals"

@str = private unnamed_addr constant [9 x i8] c"negative\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"zero\00", align 1
@str.2 = private unnamed_addr constant [9 x i8] c"positive\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@print_fmt_str.3 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define ptr @classify(double %n) {
entry:
  %n1 = alloca double, align 8
  store double %n, ptr %n1, align 8
  %load_n = load double, ptr %n1, align 8
  %cmptmp = fcmp olt double %load_n, 0.000000e+00
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  br label %if_merge

if_else:                                          ; preds = %entry
  %load_n2 = load double, ptr %n1, align 8
  %cmptmp3 = fcmp oeq double %load_n2, 0.000000e+00
  br i1 %cmptmp3, label %if_then4, label %if_else5

if_merge:                                         ; preds = %if_merge6, %if_then
  %iftmp_str7 = phi ptr [ @str, %if_then ], [ %iftmp_str, %if_merge6 ]
  ret ptr %iftmp_str7

if_then4:                                         ; preds = %if_else
  br label %if_merge6

if_else5:                                         ; preds = %if_else
  br label %if_merge6

if_merge6:                                        ; preds = %if_else5, %if_then4
  %iftmp_str = phi ptr [ @str.1, %if_then4 ], [ @str.2, %if_else5 ]
  br label %if_merge
}

define i32 @main() {
entry:
  %call_classify = call ptr @classify(double -5.000000e+00)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %call_classify)
  %call_classify1 = call ptr @classify(double 0.000000e+00)
  %print2 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.3, ptr %call_classify1)
  %call_classify3 = call ptr @classify(double 4.200000e+01)
  %print4 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr %call_classify3)
  ret i32 0
}

declare i32 @printf(ptr, ...)
