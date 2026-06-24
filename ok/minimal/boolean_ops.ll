; ModuleID = 'boolean_ops'
source_filename = "boolean_ops"

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
@str.11 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.12 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.13 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.14 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.15 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.16 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.17 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.18 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.19 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.20 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.21 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.22 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define i32 @main() {
entry:
  br i1 true, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  br i1 true, label %if_then2, label %if_else3

if_then2:                                         ; preds = %if_merge
  %print5 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge4

if_else3:                                         ; preds = %if_merge
  %print6 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge4

if_merge4:                                        ; preds = %if_else3, %if_then2
  %iftmp_str7 = phi ptr [ @str.3, %if_then2 ], [ @str.5, %if_else3 ]
  br i1 true, label %if_then8, label %if_else9

if_then8:                                         ; preds = %if_merge4
  %print11 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.8, ptr @str.7)
  br label %if_merge10

if_else9:                                         ; preds = %if_merge4
  %print12 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.10, ptr @str.9)
  br label %if_merge10

if_merge10:                                       ; preds = %if_else9, %if_then8
  %iftmp_str13 = phi ptr [ @str.7, %if_then8 ], [ @str.9, %if_else9 ]
  br i1 true, label %if_then14, label %if_else15

if_then14:                                        ; preds = %if_merge10
  %print17 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.12, ptr @str.11)
  br label %if_merge16

if_else15:                                        ; preds = %if_merge10
  %print18 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.14, ptr @str.13)
  br label %if_merge16

if_merge16:                                       ; preds = %if_else15, %if_then14
  %iftmp_str19 = phi ptr [ @str.11, %if_then14 ], [ @str.13, %if_else15 ]
  br i1 true, label %if_then20, label %if_else21

if_then20:                                        ; preds = %if_merge16
  %print23 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.16, ptr @str.15)
  br label %if_merge22

if_else21:                                        ; preds = %if_merge16
  %print24 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.18, ptr @str.17)
  br label %if_merge22

if_merge22:                                       ; preds = %if_else21, %if_then20
  %iftmp_str25 = phi ptr [ @str.15, %if_then20 ], [ @str.17, %if_else21 ]
  br i1 true, label %if_then26, label %if_else27

if_then26:                                        ; preds = %if_merge22
  %print29 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.20, ptr @str.19)
  br label %if_merge28

if_else27:                                        ; preds = %if_merge22
  %print30 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.22, ptr @str.21)
  br label %if_merge28

if_merge28:                                       ; preds = %if_else27, %if_then26
  %iftmp_str31 = phi ptr [ @str.19, %if_then26 ], [ @str.21, %if_else27 ]
  ret i32 0
}

declare i32 @printf(ptr, ...)
