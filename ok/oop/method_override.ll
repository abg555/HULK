; ModuleID = 'method_override'
source_filename = "method_override"

%vtable.FancyPrinter = type { i64, ptr, ptr }
%vtable.Printer = type { i64, ptr, ptr }
%obj.Printer = type { ptr, ptr }
%obj.FancyPrinter = type { ptr, ptr, ptr }

@vtable.FancyPrinter = constant %vtable.FancyPrinter { i64 0, ptr @FancyPrinter.format, ptr @Printer.print_msg }
@vtable.Printer = constant %vtable.Printer { i64 1, ptr @Printer.format, ptr @Printer.print_msg }
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str = private unnamed_addr constant [8 x i8] c"[INFO] \00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"test\00", align 1
@str.2 = private unnamed_addr constant [12 x i8] c"[INFO] test\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.7 = private unnamed_addr constant [4 x i8] c">> \00", align 1
@str.8 = private unnamed_addr constant [4 x i8] c" <<\00", align 1
@str.9 = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.10 = private unnamed_addr constant [12 x i8] c">> hello <<\00", align 1
@str.11 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.12 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.13 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.14 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.15 = private unnamed_addr constant [3 x i8] c"* \00", align 1
@str.16 = private unnamed_addr constant [3 x i8] c" *\00", align 1
@str.17 = private unnamed_addr constant [2 x i8] c"x\00", align 1
@str.18 = private unnamed_addr constant [6 x i8] c"* x *\00", align 1
@str.19 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.20 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.21 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.22 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define ptr @Printer.format(ptr %self, ptr %msg) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %msg2 = alloca ptr, align 8
  store ptr %msg, ptr %msg2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %pfx_field = getelementptr inbounds %obj.Printer, ptr %load_self, i32 0, i32 1
  %load_pfx = load ptr, ptr %pfx_field, align 8
  %load_msg = load ptr, ptr %msg2, align 8
  %concat = call ptr @hulk_concat(ptr %load_pfx, ptr %load_msg)
  ret ptr %concat
}

define ptr @Printer.print_msg(ptr %self, ptr %msg) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %msg2 = alloca ptr, align 8
  store ptr %msg, ptr %msg2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %vtable_ptr = getelementptr inbounds %obj.Printer, ptr %load_self, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.Printer, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %load_msg = load ptr, ptr %msg2, align 8
  %call_Printer_format = call ptr %method_load(ptr %load_self, ptr %load_msg)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %call_Printer_format)
  ret ptr %call_Printer_format
}

define ptr @FancyPrinter.format(ptr %self, ptr %msg) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %msg2 = alloca ptr, align 8
  store ptr %msg, ptr %msg2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %load_msg = load ptr, ptr %msg2, align 8
  %call_base_format = call ptr @Printer.format(ptr %load_self, ptr %load_msg)
  %load_self3 = load ptr, ptr %self1, align 8
  %sfx_field = getelementptr inbounds %obj.FancyPrinter, ptr %load_self3, i32 0, i32 2
  %load_sfx = load ptr, ptr %sfx_field, align 8
  %concat = call ptr @hulk_concat(ptr %call_base_format, ptr %load_sfx)
  ret ptr %concat
}

declare ptr @hulk_concat(ptr, ptr)

declare i32 @printf(ptr, ...)

define i32 @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Printer, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Printer, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Printer, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %prefix = alloca ptr, align 8
  store ptr @str, ptr %prefix, align 8
  %load_prefix = load ptr, ptr %prefix, align 8
  %pfx_field = getelementptr inbounds %obj.Printer, ptr %obj_alloc, i32 0, i32 1
  store ptr %load_prefix, ptr %pfx_field, align 8
  %p = alloca ptr, align 8
  store ptr %obj_alloc, ptr %p, align 8
  %load_p = load ptr, ptr %p, align 8
  %vtable_ptr = getelementptr inbounds %obj.Printer, ptr %load_p, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.Printer, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_Printer_format = call ptr %method_load(ptr %load_p, ptr @str.1)
  %streqtmp = call i32 @strcmp(ptr %call_Printer_format, ptr @str.2)
  %strcmptmp = icmp eq i32 %streqtmp, 0
  br i1 %strcmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str.3, %if_then ], [ @str.5, %if_else ]
  %obj_alloc2 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.FancyPrinter, ptr null, i32 1) to i64))
  %vtable_slot3 = getelementptr inbounds %obj.FancyPrinter, ptr %obj_alloc2, i32 0, i32 0
  store ptr @vtable.FancyPrinter, ptr %vtable_slot3, align 8
  %self4 = alloca ptr, align 8
  store ptr %obj_alloc2, ptr %self4, align 8
  %prefix5 = alloca ptr, align 8
  store ptr @str.7, ptr %prefix5, align 8
  %suffix = alloca ptr, align 8
  store ptr @str.8, ptr %suffix, align 8
  %load_prefix6 = load ptr, ptr %prefix5, align 8
  %prefix7 = alloca ptr, align 8
  store ptr %load_prefix6, ptr %prefix7, align 8
  %load_prefix8 = load ptr, ptr %prefix7, align 8
  %pfx_field9 = getelementptr inbounds %obj.FancyPrinter, ptr %obj_alloc2, i32 0, i32 1
  store ptr %load_prefix8, ptr %pfx_field9, align 8
  %load_suffix = load ptr, ptr %suffix, align 8
  %sfx_field = getelementptr inbounds %obj.FancyPrinter, ptr %obj_alloc2, i32 0, i32 2
  store ptr %load_suffix, ptr %sfx_field, align 8
  %fp = alloca ptr, align 8
  store ptr %obj_alloc2, ptr %fp, align 8
  %load_fp = load ptr, ptr %fp, align 8
  %vtable_ptr10 = getelementptr inbounds %obj.FancyPrinter, ptr %load_fp, i32 0, i32 0
  %vtable_load11 = load ptr, ptr %vtable_ptr10, align 8
  %method_ptr12 = getelementptr inbounds %vtable.FancyPrinter, ptr %vtable_load11, i32 0, i32 1
  %method_load13 = load ptr, ptr %method_ptr12, align 8
  %call_FancyPrinter_format = call ptr %method_load13(ptr %load_fp, ptr @str.9)
  %streqtmp14 = call i32 @strcmp(ptr %call_FancyPrinter_format, ptr @str.10)
  %strcmptmp15 = icmp eq i32 %streqtmp14, 0
  br i1 %strcmptmp15, label %if_then16, label %if_else17

if_then16:                                        ; preds = %if_merge
  %print19 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.12, ptr @str.11)
  br label %if_merge18

if_else17:                                        ; preds = %if_merge
  %print20 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.14, ptr @str.13)
  br label %if_merge18

if_merge18:                                       ; preds = %if_else17, %if_then16
  %iftmp_str21 = phi ptr [ @str.11, %if_then16 ], [ @str.13, %if_else17 ]
  %obj_alloc22 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.FancyPrinter, ptr null, i32 1) to i64))
  %vtable_slot23 = getelementptr inbounds %obj.FancyPrinter, ptr %obj_alloc22, i32 0, i32 0
  store ptr @vtable.FancyPrinter, ptr %vtable_slot23, align 8
  %self24 = alloca ptr, align 8
  store ptr %obj_alloc22, ptr %self24, align 8
  %prefix25 = alloca ptr, align 8
  store ptr @str.15, ptr %prefix25, align 8
  %suffix26 = alloca ptr, align 8
  store ptr @str.16, ptr %suffix26, align 8
  %load_prefix27 = load ptr, ptr %prefix25, align 8
  %prefix28 = alloca ptr, align 8
  store ptr %load_prefix27, ptr %prefix28, align 8
  %load_prefix29 = load ptr, ptr %prefix28, align 8
  %pfx_field30 = getelementptr inbounds %obj.FancyPrinter, ptr %obj_alloc22, i32 0, i32 1
  store ptr %load_prefix29, ptr %pfx_field30, align 8
  %load_suffix31 = load ptr, ptr %suffix26, align 8
  %sfx_field32 = getelementptr inbounds %obj.FancyPrinter, ptr %obj_alloc22, i32 0, i32 2
  store ptr %load_suffix31, ptr %sfx_field32, align 8
  %base = alloca ptr, align 8
  store ptr %obj_alloc22, ptr %base, align 8
  %load_base = load ptr, ptr %base, align 8
  %vtable_ptr33 = getelementptr inbounds %obj.Printer, ptr %load_base, i32 0, i32 0
  %vtable_load34 = load ptr, ptr %vtable_ptr33, align 8
  %method_ptr35 = getelementptr inbounds %vtable.Printer, ptr %vtable_load34, i32 0, i32 1
  %method_load36 = load ptr, ptr %method_ptr35, align 8
  %call_Printer_format37 = call ptr %method_load36(ptr %load_base, ptr @str.17)
  %streqtmp38 = call i32 @strcmp(ptr %call_Printer_format37, ptr @str.18)
  %strcmptmp39 = icmp eq i32 %streqtmp38, 0
  br i1 %strcmptmp39, label %if_then40, label %if_else41

if_then40:                                        ; preds = %if_merge18
  %print43 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.20, ptr @str.19)
  br label %if_merge42

if_else41:                                        ; preds = %if_merge18
  %print44 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.22, ptr @str.21)
  br label %if_merge42

if_merge42:                                       ; preds = %if_else41, %if_then40
  %iftmp_str45 = phi ptr [ @str.19, %if_then40 ], [ @str.21, %if_else41 ]
  ret i32 0
}

declare ptr @malloc(i64)

declare i32 @strcmp(ptr, ptr)
